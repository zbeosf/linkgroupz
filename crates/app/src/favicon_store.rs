use base64::engine::general_purpose::STANDARD;
use base64::Engine;
use image::imageops::FilterType;
use image::ImageFormat;
use linkgroupz_core::{write_atomic, FaviconParser, LinkStore};
use std::collections::HashMap;
use std::fs;
use std::path::PathBuf;
use std::sync::{Arc, Mutex};
use std::time::{Duration, SystemTime};

/// 사이트 아이콘을 사이트(호스트)별로 한 번 받아 앱 데이터 폴더(favicons/)에 저장해 두고 쓴다.
///
/// 1. 사이트의 /favicon.ico
/// 2. 잘 알려진 아이콘 경로 (Jira · Confluence 등, FaviconParser::KNOWN_ICON_PATHS)
/// 3. 없으면 페이지 HTML 의 <link rel="icon">
///
/// 받지 못하면 하루 동안 다시 시도하지 않는다 (지구본 아이콘 표시).
pub struct FaviconStore {
    folder: PathBuf,
    client: reqwest::Client,
    /// 호스트 → 아이콘(data URL). None 은 받지 못한 호스트.
    loaded: Mutex<HashMap<String, Option<Arc<str>>>>,
    /// 같은 호스트를 동시에 여러 번 받지 않도록 호스트마다 하나씩 두는 잠금
    in_flight: Mutex<HashMap<String, Arc<tokio::sync::Mutex<()>>>>,
}

/// 받아 둘 아이콘 한 변 픽셀
const ICON_SIZE: u32 = 64;
/// 받지 못한 사이트를 다시 시도하기까지
const RETRY_INTERVAL: Duration = Duration::from_secs(24 * 60 * 60);
/// 아이콘 찾는 방식이 바뀌면 올린다. 예전 방식으로 실패한 기록(.none)은 무시하고 다시 시도한다.
const LOOKUP_VERSION: &str = "2";
/// HTML 은 앞부분만 본다 (<head> 안에 있다)
const HTML_LIMIT: usize = 512 * 1024;
/// 아이콘 파일 크기 한도
const ICON_LIMIT: usize = 4 * 1024 * 1024;

impl FaviconStore {
    pub fn new() -> Self {
        let client = reqwest::Client::builder()
            .timeout(Duration::from_secs(8))
            .user_agent("Mozilla/5.0 (Macintosh) LinkGroupz")
            .build()
            .unwrap_or_default();
        Self {
            folder: LinkStore::data_directory().join("favicons"),
            client,
            loaded: Mutex::new(HashMap::new()),
            in_flight: Mutex::new(HashMap::new()),
        }
    }

    fn icon_file(&self, key: &str) -> PathBuf {
        self.folder.join(format!("{key}.png"))
    }

    fn miss_file(&self, key: &str) -> PathBuf {
        self.folder.join(format!("{key}.none"))
    }

    fn remembered(&self, key: &str) -> Option<Option<Arc<str>>> {
        crate::state::lock(&self.loaded).get(key).cloned()
    }

    fn remember(&self, key: &str, icon: Option<Arc<str>>) -> Option<Arc<str>> {
        crate::state::lock(&self.loaded).insert(key.to_owned(), icon.clone());
        icon
    }

    /// 페이지 주소의 사이트 아이콘 (data URL). 없으면 None.
    pub async fn icon(&self, page_url: &str) -> Option<Arc<str>> {
        let key = FaviconParser::cache_key(page_url)?;
        if let Some(known) = self.remembered(&key) {
            return known;
        }
        let gate = crate::state::lock(&self.in_flight).entry(key.clone()).or_default().clone();
        let _turn = gate.lock().await;
        if let Some(known) = self.remembered(&key) {
            return known;
        }

        // 디스크에 받아 둔 것
        if let Ok(png) = fs::read(self.icon_file(&key)) {
            return self.remember(&key, Some(data_url(&png)));
        }
        if self.recently_missed(&key) {
            return self.remember(&key, None);
        }

        let fetched = self.fetch(page_url).await;
        let _ = fs::create_dir_all(&self.folder);
        match fetched {
            Some(png) => {
                let _ = write_atomic(&self.icon_file(&key), &png);
                let _ = fs::remove_file(self.miss_file(&key));
                self.remember(&key, Some(data_url(&png)))
            }
            None => {
                let _ = fs::write(self.miss_file(&key), LOOKUP_VERSION);
                self.remember(&key, None)
            }
        }
    }

    /// 지난 하루 안에 (지금 방식으로) 받지 못한 기록이 있는지
    fn recently_missed(&self, key: &str) -> bool {
        let file = self.miss_file(key);
        let Ok(modified) = fs::metadata(&file).and_then(|metadata| metadata.modified()) else {
            return false;
        };
        let age = SystemTime::now().duration_since(modified).unwrap_or(Duration::ZERO);
        age < RETRY_INTERVAL && fs::read_to_string(&file).is_ok_and(|version| version == LOOKUP_VERSION)
    }

    async fn fetch(&self, page_url: &str) -> Option<Vec<u8>> {
        if let Some(url) = FaviconParser::default_icon_url(page_url) {
            if let Some(png) = self.download(&url).await {
                return Some(png);
            }
        }
        for url in FaviconParser::known_icon_urls(page_url) {
            if let Some(png) = self.download(&url).await {
                return Some(png);
            }
        }

        let response = self.client.get(page_url).send().await.ok()?;
        let final_url = response.url().to_string();
        let body = read_limited(response, HTML_LIMIT).await?;
        // UTF-8 이 아닌 페이지도 <link> 태그(ASCII)는 읽을 수 있다.
        let html = String::from_utf8_lossy(&body);
        for candidate in FaviconParser::icon_urls(&html, &final_url).into_iter().take(4) {
            if let Some(png) = self.download(&candidate).await {
                return Some(png);
            }
        }
        None
    }

    /// 아이콘 하나를 받아 64×64 PNG 로 바꾼다. 받지 못했거나 그림이 아니면 None.
    async fn download(&self, url: &str) -> Option<Vec<u8>> {
        let bytes = match url.strip_prefix("data:") {
            Some(data) => decode_data_url(data)?,
            None => {
                let response = self.client.get(url).send().await.ok()?;
                if !response.status().is_success() {
                    return None;
                }
                read_limited(response, ICON_LIMIT).await?
            }
        };
        normalize_icon(&bytes)
    }
}

/// 응답 본문을 limit 바이트까지만 읽는다.
async fn read_limited(mut response: reqwest::Response, limit: usize) -> Option<Vec<u8>> {
    let mut body = Vec::new();
    while body.len() < limit {
        match response.chunk().await {
            Ok(Some(chunk)) => body.extend_from_slice(&chunk),
            Ok(None) => break,
            Err(_) => return None,
        }
    }
    body.truncate(limit);
    Some(body)
}

/// `data:` 뒤의 내용(`image/png;base64,....`)을 바이트로 푼다. base64 만 받는다.
fn decode_data_url(data: &str) -> Option<Vec<u8>> {
    let (header, payload) = data.split_once(',')?;
    if !header.to_ascii_lowercase().ends_with(";base64") {
        return None;
    }
    STANDARD.decode(payload.trim()).ok()
}

fn normalize_icon(bytes: &[u8]) -> Option<Vec<u8>> {
    if bytes.is_empty() {
        return None;
    }
    let image = image::load_from_memory(bytes).ok()?;
    if image.width() == 0 || image.height() == 0 {
        return None;
    }
    let resized = image.resize_exact(ICON_SIZE, ICON_SIZE, FilterType::Lanczos3).into_rgba8();
    let mut png = std::io::Cursor::new(Vec::new());
    resized.write_to(&mut png, ImageFormat::Png).ok()?;
    Some(png.into_inner())
}

pub fn data_url(png: &[u8]) -> Arc<str> {
    format!("data:image/png;base64,{}", STANDARD.encode(png)).into()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn data_url_icons() {
        let mut png = std::io::Cursor::new(Vec::new());
        image::RgbaImage::from_pixel(10, 30, image::Rgba([10, 200, 30, 255])).write_to(&mut png, ImageFormat::Png).unwrap();
        let encoded = STANDARD.encode(png.get_ref());
        let decoded = decode_data_url(&format!("image/png;base64,{encoded}")).expect("base64 data URL");
        let normalized = normalize_icon(&decoded).expect("PNG");
        let image = image::load_from_memory(&normalized).unwrap();
        assert_eq!((image.width(), image.height()), (ICON_SIZE, ICON_SIZE), "64px 정사각형으로 맞춘다");
        assert!(decode_data_url("image/svg+xml,%3Csvg/%3E").is_none(), "base64 가 아닌 data URL");
        assert!(normalize_icon(b"<html>not found</html>").is_none(), "그림이 아닌 응답");
        assert!(normalize_icon(b"").is_none(), "빈 응답");
        assert!(data_url(&normalized).starts_with("data:image/png;base64,"));
    }
}
