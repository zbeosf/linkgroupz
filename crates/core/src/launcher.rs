use crate::base64url::Base64Url;
use crate::brand;
use crate::browser::Browser;
use crate::models::{LinkGroup, SavedLink};
use crate::platform;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;
use url::Url;

#[derive(Debug, thiserror::Error)]
pub enum LauncherError {
    #[error("{}", crate::tr!("{} 을(를) 찾지 못했습니다.", "Couldn't find {}.", .0.full_name()))]
    BrowserNotFound(Browser),
    #[error("{}", crate::tr!("그룹에 열 페이지가 없습니다.", "The group has no pages to open."))]
    EmptyGroup,
    #[error("{}", crate::tr!("열 수 있는 주소가 아닙니다 (http · https · file 주소만 엽니다): {}", "This address can't be opened (only http, https and file addresses): {}", .0))]
    UnsupportedUrl(String),
    #[error("{}", crate::tr!("{} 을(를) 실행하지 못했습니다: {}", "Couldn't start {}: {}", .0.full_name(), .1))]
    Launch(Browser, String),
    #[error("{}", crate::tr!("회사 관리 정책으로 {}의 {}이 꺼져 있어 열지 않았습니다 (열면 일반 창으로 열립니다). 이 그룹을 다른 브라우저로 바꾸면 시크릿 창으로 열 수 있습니다.", "Not opened because {}'s {} is turned off by your organization's policy (it would open in a normal window). Switch this group to another browser to open it privately.", .0.display_name(), .0.private_window_name()))]
    PrivateModeDisabled(Browser),
}

/// 확장이 가로채는 마커 URL 에 싣는 내용
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct MarkerPayload {
    pub name: String,
    pub color: String,
    pub urls: Vec<String>,
}

/// 브라우저 확장(chrome-extension/background.js)이 가로채는 마커 URL.
/// `.invalid` 는 접속되지 않는 예약 도메인이라, 확장이 없을 때 다른 사이트로 새어 나가지 않는다.
pub struct TabGroupMarker;

impl TabGroupMarker {
    /// LinkGroupz 확장이 가로채는 호스트
    pub const HOST: &'static str = brand::MARKER_HOST;

    /// LinkGroupz 확장이 알아듣는 마커 URL
    pub fn url(group: &LinkGroup) -> String {
        let payload = MarkerPayload {
            name: group.name.clone(),
            color: group.color.chrome_color().raw_value().to_owned(),
            urls: group.links.iter().filter(|link| link.url.is_openable()).map(|link| link.url.as_str().to_owned()).collect(),
        };
        let json = serde_json::to_vec(&payload).expect("문자열만 든 값은 항상 JSON 으로 바뀐다");
        format!("https://{}/open#{}", Self::HOST, Base64Url::encode(&json))
    }

    pub fn decode(url: &str) -> Option<MarkerPayload> {
        let url = Url::parse(url).ok()?;
        if url.host_str()? != Self::HOST {
            return None;
        }
        let data = Base64Url::decode(url.fragment()?)?;
        serde_json::from_slice(&data).ok()
    }
}

/// 그룹 · 페이지를 브라우저(Chrome · Edge · Whale)에서 연다. 그룹은 그룹에 지정된 브라우저로 연다.
pub struct ChromeLauncher;

impl ChromeLauncher {
    pub fn open_link(link: &SavedLink, browser: Browser) -> Result<(), LauncherError> {
        if !link.url.is_openable() {
            return Err(LauncherError::UnsupportedUrl(link.url.as_str().to_owned()));
        }
        let app = Self::require_app(browser)?;
        platform::open_urls(&app, &[link.url.as_str().to_owned()]).map_err(|error| Self::launch_error(browser, error))
    }

    /// 페이지들을 새 창 하나에 연다. 브라우저가 이미 떠 있어도 지금 창에 더하지 않는다.
    pub fn open_links_in_new_window(links: &[SavedLink], browser: Browser) -> Result<(), LauncherError> {
        if links.is_empty() {
            return Err(LauncherError::EmptyGroup);
        }
        Self::require_openable(links)?;
        let app = Self::require_app(browser)?;
        platform::launch_with_arguments(&app, &Self::new_window_arguments(links)).map_err(|error| Self::launch_error(browser, error))
    }

    /// 그룹을 새 창에 연다. 확장이 있으면 새 창에서 탭 그룹으로 연다 (확장은 마커 탭이 있는 창에 탭 그룹을 만든다).
    pub fn open_group_in_new_window(group: &LinkGroup, tab_group: bool) -> Result<(), LauncherError> {
        if group.links.is_empty() {
            return Err(LauncherError::EmptyGroup);
        }
        Self::require_openable(&group.links)?;
        let browser = group.browser;
        let app = Self::require_app(browser)?;
        let arguments = if tab_group {
            vec!["--new-window".to_owned(), TabGroupMarker::url(group)]
        } else {
            Self::new_window_arguments(&group.links)
        };
        platform::launch_with_arguments(&app, &arguments).map_err(|error| Self::launch_error(browser, error))
    }

    /// tab_group 이면 그 브라우저의 LinkGroupz 확장이 탭 그룹으로 열고, 아니면 새 창 하나에 모두 연다.
    pub fn open_group(group: &LinkGroup, tab_group: bool) -> Result<(), LauncherError> {
        if group.links.is_empty() {
            return Err(LauncherError::EmptyGroup);
        }
        Self::require_openable(&group.links)?;
        let browser = group.browser;
        let app = Self::require_app(browser)?;
        let result = if tab_group {
            platform::open_urls(&app, &[TabGroupMarker::url(group)])
        } else {
            // 실행 중인 브라우저에도 새 창으로 전달된다.
            platform::launch_with_arguments(&app, &Self::new_window_arguments(&group.links))
        };
        result.map_err(|error| Self::launch_error(browser, error))
    }

    /// 페이지들을 새 시크릿 창(Edge 는 InPrivate 창) 하나에 연다. 확장 · 탭 그룹 없이 연다 (확장은 보통 시크릿 창에서 꺼져 있다).
    pub fn open_private(links: &[SavedLink], browser: Browser) -> Result<(), LauncherError> {
        if links.is_empty() {
            return Err(LauncherError::EmptyGroup);
        }
        Self::require_openable(links)?;
        let app = Self::require_app(browser)?;
        // 정책으로 막혀 있으면 브라우저가 비공개 창 대신 일반 창으로 연다. 비공개로 열려던 페이지가 일반 기록에 남지 않도록 열지 않는다.
        if browser.private_mode_disabled() {
            return Err(LauncherError::PrivateModeDisabled(browser));
        }
        platform::launch_with_arguments(&app, &Self::private_window_arguments(links, browser))
            .map_err(|error| Self::launch_error(browser, error))
    }

    /// 열 수 있는 주소만 실행 인자로 넘긴다 (`LinkUrl::is_openable`).
    pub fn new_window_arguments(links: &[SavedLink]) -> Vec<String> {
        std::iter::once("--new-window".to_owned()).chain(Self::openable_urls(links)).collect()
    }

    pub fn private_window_arguments(links: &[SavedLink], browser: Browser) -> Vec<String> {
        std::iter::once(browser.private_window_argument().to_owned()).chain(Self::openable_urls(links)).collect()
    }

    fn openable_urls(links: &[SavedLink]) -> impl Iterator<Item = String> + '_ {
        links.iter().filter(|link| link.url.is_openable()).map(|link| link.url.as_str().to_owned())
    }

    /// 열 수 있는 주소가 하나도 없으면 첫 번째 주소를 알려 주는 오류
    fn require_openable(links: &[SavedLink]) -> Result<(), LauncherError> {
        match links.iter().find(|link| link.url.is_openable()) {
            Some(_) => Ok(()),
            None => Err(LauncherError::UnsupportedUrl(links[0].url.as_str().to_owned())),
        }
    }

    fn require_app(browser: Browser) -> Result<PathBuf, LauncherError> {
        browser.app_path().ok_or(LauncherError::BrowserNotFound(browser))
    }

    fn launch_error(browser: Browser, error: std::io::Error) -> LauncherError {
        LauncherError::Launch(browser, error.to_string())
    }
}
