use regex::Regex;
use std::sync::LazyLock;
use url::Url;

/// 사이트 아이콘을 받을 주소 후보.
///
/// 아이콘은 그 사이트에서만 받는다. 외부 아이콘 서비스(예: Google)를 쓰면 사내 주소가 밖으로 나가기 때문이다.
pub struct FaviconParser;

static LINK_TAG: LazyLock<Regex> = LazyLock::new(|| Regex::new(r"(?i)<link\b[^>]*>").expect("고정된 정규식"));

impl FaviconParser {
    /// /favicon.ico 가 없을 때 시도하는 잘 알려진 아이콘 경로.
    /// 사내 Jira · Confluence 처럼 로그인 전에는 첫 페이지가 SSO 로그인 화면으로 넘어가 HTML 에서 아이콘을 못 찾는 사이트도,
    /// 이 정적 파일들은 로그인 없이 받을 수 있다.
    pub const KNOWN_ICON_PATHS: [&'static str; 5] = [
        "/images/fav-jsw.png",                  // Jira Software
        "/images/logo/confluence_48_trans.png", // Confluence
        "/images/icons/favicon.png",            // Jira · Confluence 공통 (16px)
        "/favicon.png",
        "/apple-touch-icon.png",
    ];

    /// 사이트(호스트) 단위 캐시 키. 포트가 있으면 포함한다.
    pub fn cache_key(url: &str) -> Option<String> {
        let url = Url::parse(url).ok()?;
        let host = url.host_str().filter(|host| !host.is_empty())?.to_lowercase();
        Some(match url.port() {
            Some(port) => format!("{host}_{port}"),
            None => host,
        })
    }

    /// 사이트 루트의 /favicon.ico
    pub fn default_icon_url(url: &str) -> Option<String> {
        Self::site_url(url, "/favicon.ico")
    }

    pub fn known_icon_urls(url: &str) -> Vec<String> {
        Self::KNOWN_ICON_PATHS.iter().filter_map(|path| Self::site_url(url, path)).collect()
    }

    /// 같은 사이트의 다른 경로 (쿼리 · 조각 · 계정 정보는 뗀다). http(s) 가 아니면 None.
    fn site_url(url: &str, path: &str) -> Option<String> {
        let mut url = Url::parse(url).ok()?;
        if !["http", "https"].contains(&url.scheme()) {
            return None;
        }
        url.set_path(path);
        url.set_query(None);
        url.set_fragment(None);
        let _ = url.set_username("");
        let _ = url.set_password(None);
        Some(url.to_string())
    }

    /// HTML 의 <link rel="icon" ...> 들을 우선순위 순으로. apple-touch-icon 은 크기가 커서 뒤로 둔다.
    pub fn icon_urls(html: &str, base_url: &str) -> Vec<String> {
        let Ok(base) = Url::parse(base_url) else {
            return Vec::new();
        };
        let mut preferred = Vec::new();
        let mut touch_icons = Vec::new();
        for tag in LINK_TAG.find_iter(html).map(|found| found.as_str()) {
            let (Some(rel), Some(href)) = (Self::attribute("rel", tag), Self::attribute("href", tag)) else {
                continue;
            };
            let Ok(url) = base.join(&href) else {
                continue;
            };
            if !["http", "https", "data"].contains(&url.scheme()) {
                continue;
            }
            let rel = rel.to_lowercase();
            let rels: Vec<&str> = rel.split(' ').filter(|part| !part.is_empty()).collect();
            if rels.contains(&"icon") {
                preferred.push(url.to_string());
            } else if rels.contains(&"apple-touch-icon") || rels.contains(&"apple-touch-icon-precomposed") {
                touch_icons.push(url.to_string());
            }
        }
        preferred.extend(touch_icons);
        preferred
    }

    /// 태그 안의 속성 값 (따옴표가 있든 없든)
    fn attribute(name: &str, tag: &str) -> Option<String> {
        let pattern = format!(r#"(?i)\b{}\s*=\s*(?:"([^"]*)"|'([^']*)'|([^\s>]+))"#, regex::escape(name));
        let captures = Regex::new(&pattern).ok()?.captures(tag)?;
        (1..=3).find_map(|group| captures.get(group)).map(|value| value.as_str().trim().to_owned())
    }
}
