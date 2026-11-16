//! 버전 비교와 업데이트 창을 띄울지에 대한 규칙.
//!
//! 새 버전을 확인하고 내려받아 설치하는 일은 앱(crates/app/src/update.rs)이 Tauri 업데이터로 한다.

use crate::brand;
use serde::{Serialize, Serializer};
use std::cmp::Ordering;
use std::collections::HashMap;
use std::fmt;
use std::hash::{Hash, Hasher};
use std::path::Path;

/// 배포 파일이 올라오는 GitHub 저장소와, 받은 파일의 서명을 확인할 공개 키.
///
/// 둘 다 소스에 두지 않는다. 빌드할 때 환경 변수나 작업 공간 맨 위의 `update-source.env`
/// (git 에 올리지 않는 파일, `update-source.env.example` 참고)에서 읽어 앱에 넣고,
/// 없으면 자동 업데이트 없이 빌드된다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct UpdateSource {
    repository: String,
    public_key: String,
}

impl UpdateSource {
    /// `update-source.env` (작업 공간 맨 위)
    pub const FILE_NAME: &'static str = "update-source.env";

    /// repository 는 `소유자/이름`, public_key 는 `tauri signer generate` 가 만든 공개 키(.pub 파일의 내용)
    pub fn new(repository: &str, public_key: &str) -> Option<Self> {
        let repository = repository.trim();
        let public_key = public_key.trim();
        let is_name = |part: &str| {
            !part.is_empty() && part != "." && part != ".." && part.chars().all(|c| c.is_ascii_alphanumeric() || matches!(c, '-' | '_' | '.'))
        };
        let (owner, name) = repository.split_once('/')?;
        if !is_name(owner) || !is_name(name) || public_key.is_empty() || public_key.chars().any(char::is_whitespace) {
            return None;
        }
        Some(Self { repository: repository.to_owned(), public_key: public_key.to_owned() })
    }

    /// `이름=값` 줄로 된 설정 파일의 내용을 읽는다 (`#` 로 시작하는 줄은 설명).
    pub fn parse_settings(text: &str) -> HashMap<String, String> {
        text.lines()
            .map(str::trim)
            .filter(|line| !line.is_empty() && !line.starts_with('#'))
            .filter_map(|line| line.split_once('='))
            .map(|(name, value)| (name.trim().to_owned(), value.trim().trim_matches('"').to_owned()))
            .collect()
    }

    /// 빌드 도구용: 환경 변수 → 작업 공간의 `update-source.env` 순으로 찾는다.
    pub fn load(workspace_root: &Path) -> Option<Self> {
        let from_file = Self::parse_settings(&std::fs::read_to_string(workspace_root.join(Self::FILE_NAME)).unwrap_or_default());
        let value = |name: &str| {
            std::env::var(name).ok().filter(|value| !value.trim().is_empty()).or_else(|| from_file.get(name).cloned())
        };
        Self::new(&value(brand::env::RELEASES_REPO)?, &value(brand::env::UPDATE_PUBKEY)?)
    }

    pub fn repository(&self) -> &str {
        &self.repository
    }

    pub fn public_key(&self) -> &str {
        &self.public_key
    }

    /// 앱이 새 버전을 확인할 때 읽는 파일 (최신 릴리스에 올린 latest.json)
    pub fn manifest_url(&self) -> String {
        format!("https://github.com/{}/releases/latest/download/latest.json", self.repository)
    }

    /// 릴리스 페이지 (브라우저로 여는 주소)
    pub fn releases_page_url(&self) -> String {
        format!("https://github.com/{}/releases", self.repository)
    }

    /// 릴리스 `tag` 에 올린 파일을 내려받는 주소
    pub fn download_url(&self, tag: &str, file_name: &str) -> String {
        format!("https://github.com/{}/releases/download/{tag}/{file_name}", self.repository)
    }
}

/// "1.4.1" 같은 버전. 숫자 칸을 앞에서부터 비교하고, 없는 칸은 0 으로 본다 (1.4 == 1.4.0).
#[derive(Clone, Debug)]
pub struct AppVersion {
    parts: Vec<u64>,
}

impl AppVersion {
    pub fn parse(text: &str) -> Option<Self> {
        let parts = text.split('.').map(|piece| piece.parse::<u64>().ok()).collect::<Option<Vec<u64>>>()?;
        Some(Self { parts })
    }

    pub fn parts(&self) -> &[u64] {
        &self.parts
    }

    /// 끝의 0 을 뗀 칸 (1.4.0 → [1, 4])
    fn significant(&self) -> &[u64] {
        let length = self.parts.iter().rposition(|part| *part != 0).map_or(0, |index| index + 1);
        &self.parts[..length]
    }
}

impl fmt::Display for AppVersion {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        let text: Vec<String> = self.parts.iter().map(u64::to_string).collect();
        f.write_str(&text.join("."))
    }
}

impl Ord for AppVersion {
    fn cmp(&self, other: &Self) -> Ordering {
        // 끝의 0 을 뗀 뒤 앞에서부터 비교하면, 모자란 칸을 0 으로 채워 비교한 것과 같다.
        self.significant().cmp(other.significant())
    }
}

impl PartialOrd for AppVersion {
    fn partial_cmp(&self, other: &Self) -> Option<Ordering> {
        Some(self.cmp(other))
    }
}

impl PartialEq for AppVersion {
    fn eq(&self, other: &Self) -> bool {
        self.cmp(other) == Ordering::Equal
    }
}

impl Eq for AppVersion {}

impl Hash for AppVersion {
    fn hash<H: Hasher>(&self, state: &mut H) {
        self.significant().hash(state);
    }
}

impl Serialize for AppVersion {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        serializer.collect_str(self)
    }
}

/// 1시간마다 확인할 때 업데이트 창을 띄울지.
/// '나중에'를 고른 버전(skipped)보다 높은 버전이 새로 올라오기 전까지는 다시 묻지 않는다.
pub struct UpdatePromptPolicy;

impl UpdatePromptPolicy {
    pub fn should_prompt(latest: &AppVersion, current: &AppVersion, skipped: Option<&AppVersion>) -> bool {
        if latest <= current {
            return false;
        }
        !skipped.is_some_and(|skipped| latest <= skipped)
    }
}
