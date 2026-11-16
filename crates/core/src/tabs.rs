use crate::browser::Browser;
use crate::models::{GroupColor, LinkUrl, SavedLink};
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

/// Chrome 탭 그룹. 확장을 거쳐 읽을 때만 알 수 있다.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeTabGroupInfo {
    pub id: i64,
    pub title: String,
    pub color: GroupColor,
    /// 브라우저에 저장만 돼 있고 지금 열려 있지 않은 그룹 (crates/core/src/saved_tab_groups.rs)
    #[serde(default)]
    pub is_closed: bool,
}

impl ChromeTabGroupInfo {
    pub fn new(id: i64, title: impl Into<String>, color: GroupColor) -> Self {
        Self { id, title: title.into(), color, is_closed: false }
    }

    pub fn display_title(&self) -> &str {
        if self.title.is_empty() {
            crate::tr!("이름 없는 그룹", "Untitled group")
        } else {
            &self.title
        }
    }
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeTab {
    /// 1 = 맨 앞 창
    pub window_index: usize,
    pub title: String,
    pub url: String,
    #[serde(default)]
    pub tab_group: Option<ChromeTabGroupInfo>,
}

impl ChromeTab {
    pub fn new(window_index: usize, title: impl Into<String>, url: impl Into<String>) -> Self {
        Self { window_index, title: title.into(), url: url.into(), tab_group: None }
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum CandidateStatus {
    Available,
    AlreadyInGroup,
    /// chrome:// 등 http(s) · file 이 아닌 주소
    Unsupported,
}

/// 가져오기 화면의 한 줄. 이미 그룹에 있거나 열 수 없는 주소는 고를 수 없다.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct TabImportCandidate {
    pub id: usize,
    pub tab: ChromeTab,
    pub url: Option<LinkUrl>,
    pub status: CandidateStatus,
}

impl TabImportCandidate {
    pub fn is_selectable(&self) -> bool {
        self.status == CandidateStatus::Available
    }

    /// 탭 순서를 지키고, 같은 URL 이 여러 탭에 열려 있으면 처음 것만 남긴다.
    pub fn make(tabs: &[ChromeTab], existing_links: &[SavedLink]) -> Vec<TabImportCandidate> {
        let existing: HashSet<String> = existing_links.iter().map(|link| Self::normalized_key(link.url.as_str())).collect();
        let mut seen = HashSet::new();
        let mut result: Vec<TabImportCandidate> = Vec::new();
        for tab in tabs {
            let supported = Url::parse(&tab.url).is_ok_and(|url| ["http", "https", "file"].contains(&url.scheme()));
            if !supported {
                result.push(TabImportCandidate {
                    id: result.len(),
                    tab: tab.clone(),
                    url: None,
                    status: CandidateStatus::Unsupported,
                });
                continue;
            }
            let key = Self::normalized_key(&tab.url);
            if !seen.insert(key.clone()) {
                continue;
            }
            let status =
                if existing.contains(&key) { CandidateStatus::AlreadyInGroup } else { CandidateStatus::Available };
            result.push(TabImportCandidate {
                id: result.len(),
                tab: tab.clone(),
                url: Some(LinkUrl::new(tab.url.clone())),
                status,
            });
        }
        result
    }

    pub fn make_link(&self, use_title_as_alias: bool) -> Option<SavedLink> {
        let url = self.url.clone()?;
        Some(SavedLink::new(url, use_title_as_alias.then_some(self.tab.title.as_str())))
    }

    /// 끝의 / 와 #fragment 차이는 같은 페이지로 본다.
    pub fn normalized_key(url: &str) -> String {
        let without_fragment = url.split_once('#').map_or(url, |(before, _)| before);
        without_fragment.trim_end_matches('/').to_lowercase()
    }
}

#[derive(Clone, Debug, PartialEq, Eq, thiserror::Error)]
pub enum TabsError {
    #[error("{}", crate::tr!("실행 중인 {name} 창이 없습니다. {name} 창을 연 뒤 다시 시도해 주세요.", "No {name} window is open. Open a {name} window and try again.", name = .0.display_name()))]
    BrowserNotRunning(Browser),
    #[error("{}", crate::tr!("브라우저 탭을 읽을 권한이 없습니다. 시스템 설정 → 개인정보 보호 및 보안 → 자동화에서 LinkGroupz 의 Google Chrome · Microsoft Edge · Whale 을 켜 주세요.", "No permission to read browser tabs. Turn on Google Chrome · Microsoft Edge · Whale for LinkGroupz in System Settings → Privacy & Security → Automation."))]
    NotAuthorized,
    #[error("{}", crate::tr!("브라우저 탭을 읽지 못했습니다: {}", "Couldn't read the browser tabs: {}", .0))]
    Script(String),
    #[error("{}", crate::tr!("열린 브라우저 창이 없습니다.", "No browser window is open."))]
    NoWindow,
    #[error("{}", crate::tr!("{name} 확장이 응답하지 않습니다. {page} 에서 LinkGroupz 확장을 새로고침해 주세요.", "The {name} extension isn't responding. Reload the LinkGroupz extension at {page}.", name = .0.display_name(), page = .0.extensions_page()))]
    ExtensionNoResponse(Browser),
    #[error("{}", crate::tr!("{name} 에 열린 탭은 {name} 에 LinkGroupz 확장이 있어야 읽을 수 있습니다. 설정의 ‘확장 설치’를 참고해 주세요.", "Reading tabs open in {name} needs the LinkGroupz extension in {name}. See “Install the Extension” in Settings.", name = .0.display_name()))]
    ExtensionRequired(Browser),
}

/// AppleScript 가 돌려준 값
#[derive(Clone, Debug, PartialEq)]
pub enum ScriptValue {
    Null,
    Bool(bool),
    Int(i64),
    Text(String),
    List(Vec<ScriptValue>),
}

impl ScriptValue {
    pub fn as_list(&self) -> Option<&[ScriptValue]> {
        match self {
            ScriptValue::List(items) => Some(items),
            _ => None,
        }
    }

    pub fn as_int(&self) -> Option<i64> {
        match self {
            ScriptValue::Int(value) => Some(*value),
            _ => None,
        }
    }

    pub fn as_text(&self) -> Option<&str> {
        match self {
            ScriptValue::Text(value) => Some(value),
            _ => None,
        }
    }
}

/// AppleScript 실행 오류 (오류 번호 · 메시지)
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScriptFailure {
    pub number: i64,
    pub message: Option<String>,
}

impl From<ScriptFailure> for TabsError {
    fn from(failure: ScriptFailure) -> Self {
        match failure.number {
            // 자동화 권한 거부 (errAEEventNotPermitted)
            -1743 => TabsError::NotAuthorized,
            // 스크립트 안에서 직접 낸 "창 없음"
            9001 => TabsError::NoWindow,
            number => TabsError::Script(failure.message.unwrap_or_else(|| crate::tr!("오류 {}", "error {}", number))),
        }
    }
}

/// 탭을 읽을 때 앱이 대신 해 주는 일.
///
/// AppleScript 는 메인 스레드에서 실행해야 하고, 탭 읽기는 기다리는 일이라 백그라운드에서 돈다.
/// 그래서 실행을 앱(메인 스레드로 넘겨 주는 쪽)에 맡긴다.
pub trait TabsHost: Send + Sync {
    /// AppleScript 를 실행한다 (macOS).
    fn run_script(&self, source: &str) -> Result<ScriptValue, ScriptFailure>;

    /// 브라우저에 마커 탭을 열어 앞 순서를 빼앗겼을 때, 앱을 다시 맨 앞으로 가져온다.
    fn reactivate_app(&self) {}
}

/// 부르는 스레드에서 바로 실행한다 (명령줄 도구 · 검사용). 메인 스레드에서 써야 한다.
pub struct DirectTabsHost;

impl TabsHost for DirectTabsHost {
    #[cfg(target_os = "macos")]
    fn run_script(&self, source: &str) -> Result<ScriptValue, ScriptFailure> {
        crate::platform::run_apple_script(source)
    }

    #[cfg(not(target_os = "macos"))]
    fn run_script(&self, _source: &str) -> Result<ScriptValue, ScriptFailure> {
        Err(ScriptFailure { number: 0, message: Some(crate::tr!("AppleScript 는 macOS 에서만 쓸 수 있습니다", "AppleScript is only available on macOS").to_owned()) })
    }
}

/// AppleScript 로 브라우저(Chrome · Edge · Whale)에 열린 탭 목록을 읽는다 (macOS).
/// 브라우저마다 처음 한 번 macOS 가 자동화 권한을 묻는다.
pub struct ChromeTabsReader;

impl ChromeTabsReader {
    pub fn source(browser: Browser) -> String {
        format!(
            r#"tell application id "{}"
    set out to {{}}
    set windowIndex to 0
    repeat with w in windows
        set windowIndex to windowIndex + 1
        repeat with t in tabs of w
            set end of out to {{windowIndex, title of t, URL of t}}
        end repeat
    end repeat
    return out
end tell"#,
            browser.bundle_identifier()
        )
    }

    pub fn read(browser: Browser, host: &dyn TabsHost) -> Result<Vec<ChromeTab>, TabsError> {
        // 꺼져 있는 브라우저를 tell 로 띄우지 않도록 먼저 확인한다.
        if !browser.is_running() {
            return Err(TabsError::BrowserNotRunning(browser));
        }
        let result = host.run_script(&Self::source(browser))?;
        Ok(Self::parse(&result))
    }

    /// {{창 번호, 제목, URL}, ...} 목록을 읽는다.
    pub fn parse(value: &ScriptValue) -> Vec<ChromeTab> {
        let Some(items) = value.as_list() else {
            return Vec::new();
        };
        items
            .iter()
            .filter_map(|item| {
                let fields = item.as_list()?;
                if fields.len() != 3 {
                    return None;
                }
                let window = usize::try_from(fields[0].as_int()?).ok()?;
                let url = fields[2].as_text()?;
                let title = fields[1].as_text().unwrap_or("");
                Some(ChromeTab::new(window, title, url))
            })
            .collect()
    }
}
