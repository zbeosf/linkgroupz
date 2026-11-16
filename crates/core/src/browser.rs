use crate::tr;
use crate::platform;
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use std::path::PathBuf;

/// 그룹을 열고 탭을 가져오는 브라우저. 모두 Chromium 기반이라 확장 · Native Messaging 방식이 같고,
/// 앱 식별자와 프로필 폴더만 다르다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Hash, PartialOrd, Ord, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Browser {
    Chrome,
    Edge,
    Whale,
}

impl Browser {
    pub const ALL: [Browser; 3] = [Browser::Chrome, Browser::Edge, Browser::Whale];

    /// 그룹에 브라우저가 지정되지 않았을 때(예전 데이터 포함)
    pub const DEFAULT: Browser = Browser::Chrome;

    /// 파일에 적는 이름 (`"chrome"`)
    pub fn raw_value(self) -> &'static str {
        match self {
            Browser::Chrome => "chrome",
            Browser::Edge => "edge",
            Browser::Whale => "whale",
        }
    }

    pub fn from_raw_value(raw: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|browser| browser.raw_value() == raw)
    }

    pub fn display_name(self) -> &'static str {
        match self {
            Browser::Chrome => "Chrome",
            Browser::Edge => "Edge",
            Browser::Whale => "Whale",
        }
    }

    pub fn full_name(self) -> &'static str {
        match self {
            Browser::Chrome => "Google Chrome",
            Browser::Edge => "Microsoft Edge",
            Browser::Whale => "Naver Whale",
        }
    }

    /// macOS 번들 식별자
    pub fn bundle_identifier(self) -> &'static str {
        match self {
            Browser::Chrome => "com.google.Chrome",
            Browser::Edge => "com.microsoft.edgemac",
            Browser::Whale => "com.naver.Whale",
        }
    }

    /// Windows 실행 파일 이름
    pub fn windows_executable(self) -> &'static str {
        match self {
            Browser::Chrome => "chrome.exe",
            Browser::Edge => "msedge.exe",
            Browser::Whale => "whale.exe",
        }
    }

    /// 비공개 창(Chrome · Whale 시크릿 모드, Edge InPrivate)으로 여는 실행 인자
    pub fn private_window_argument(self) -> &'static str {
        match self {
            Browser::Chrome | Browser::Whale => "--incognito",
            Browser::Edge => "--inprivate",
        }
    }

    /// 비공개 창을 끄는 관리 정책의 이름. 값이 1 이면 회사 등이 비공개 창을 막아 둔 것이다.
    /// (막혀 있으면 브라우저가 비공개 창 대신 일반 창으로 연다.)
    pub fn private_mode_policy(self) -> &'static str {
        match self {
            Browser::Chrome | Browser::Whale => "IncognitoModeAvailability",
            Browser::Edge => "InPrivateModeAvailability",
        }
    }

    /// 관리 정책이 비공개 창을 막아 두었는지 (macOS 관리 환경설정 · Windows 정책 레지스트리)
    pub fn private_mode_disabled(self) -> bool {
        platform::browser_policy_value(self, self.private_mode_policy()) == Some(1)
    }

    /// 비공개 창 이름 (안내 문구용)
    pub fn private_window_name(self) -> &'static str {
        match self {
            Browser::Chrome | Browser::Whale => tr!("시크릿 창", "private window"),
            Browser::Edge => tr!("InPrivate 창", "InPrivate window"),
        }
    }

    /// 확장 관리 페이지 주소
    pub fn extensions_page(self) -> &'static str {
        match self {
            Browser::Chrome => "chrome://extensions",
            Browser::Edge => "edge://extensions",
            Browser::Whale => "whale://extensions",
        }
    }

    /// 확장 관리 페이지의 "압축해제된 확장 프로그램 로드" 버튼 이름
    pub fn load_unpacked_label(self) -> &'static str {
        match self {
            Browser::Chrome => tr!("압축해제된 확장 프로그램 로드", "Load unpacked"),
            Browser::Edge => tr!("압축을 푼 확장 로드", "Load unpacked"),
            Browser::Whale => tr!("압축해제 된 확장앱 설치", "Load unpacked extension"),
        }
    }

    /// 사용자 데이터 폴더 (프로필이 있는 곳).
    /// macOS `~/Library/Application Support/Google/Chrome`, Windows `%LOCALAPPDATA%\Google\Chrome\User Data`
    pub fn user_data_directory(self) -> PathBuf {
        platform::browser_user_data_directory(self)
    }

    /// 확장이 저장하는 탭 · 탭 그룹 목록 파일 이름 (브라우저마다 따로 둔다)
    pub fn snapshot_file_name(self) -> String {
        format!("{}-snapshot.json", self.raw_value())
    }

    /// 설치된 브라우저의 위치 (macOS 는 .app 폴더, Windows 는 실행 파일)
    pub fn app_path(self) -> Option<PathBuf> {
        platform::browser_app_path(self)
    }

    pub fn is_installed(self) -> bool {
        self.app_path().is_some()
    }

    pub fn is_running(self) -> bool {
        platform::browser_is_running(self)
    }

    /// (macOS) 지금 화면에 보이는 이 브라우저의 창 번호들. 다른 운영체제는 비어 있다.
    pub fn window_numbers(self) -> std::collections::HashSet<u32> {
        platform::browser_window_numbers(self)
    }

    /// 지금 맨 앞의 앱이 이 브라우저인지 (macOS)
    pub fn is_frontmost(self) -> bool {
        platform::frontmost_bundle_identifier().as_deref() == Some(self.bundle_identifier())
    }

    /// 지금 실행 중인 브라우저가 켜진 시각
    pub fn launch_date(self) -> Option<Timestamp> {
        platform::browser_launch_date(self)
    }

    /// 앱 아이콘 PNG (목록 표시용). 설치돼 있지 않거나 읽지 못하면 None
    pub fn icon_png(self, size: u32) -> Option<Vec<u8>> {
        let app = self.app_path()?;
        platform::app_icon_png(&app, size)
    }
}

impl Default for Browser {
    fn default() -> Self {
        Self::DEFAULT
    }
}
