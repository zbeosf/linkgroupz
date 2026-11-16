//! 앱을 켤 때의 일: 로그인할 때 자동 실행, 중복 실행 막기, DMG 안에서 실행한 경우 안내.

use serde::Serialize;
use tauri::AppHandle;

/// 로그인할 때 자동 실행 상태
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum LaunchAtLoginState {
    Enabled,
    Disabled,
    /// 등록은 됐지만 시스템 설정의 로그인 항목에서 허용이 꺼져 있음 (macOS)
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    RequiresApproval,
}

/// 이 인자를 붙여 켜면 관리 창을 열지 않고 메뉴바(트레이)에만 뜬다.
/// Windows 의 시작 프로그램 등록이 이 인자를 붙인다. macOS 는 로그인 항목으로 켜졌는지를 시스템이 알려 주지만,
/// 직접 창 없이 켜고 싶을 때(스크립트 · 측정)도 쓸 수 있게 어느 운영체제에서나 받는다.
pub const AUTOSTART_ARGUMENT: &str = "--autostart";

/// DMG 를 연 볼륨(/Volumes/…) 이거나, 받은 앱을 옮기지 않고 열어 macOS 가 임시 위치(AppTranslocation)에서 실행한 경우
#[cfg_attr(not(target_os = "macos"), allow(dead_code))]
pub fn is_temporary_location(path: &str) -> bool {
    path.starts_with("/Volumes/") || path.contains("/AppTranslocation/")
}

/// 실행 중인 앱 번들(.app) 위치. 번들 밖(cargo run)이면 None.
#[cfg(target_os = "macos")]
pub fn bundle_path() -> Option<std::path::PathBuf> {
    let executable = std::env::current_exe().ok()?;
    executable.ancestors().find(|path| path.extension().is_some_and(|extension| extension == "app")).map(|path| path.to_owned())
}

/// DMG 안이나 임시 위치처럼 옮겨질 수 있는 곳에서 실행 중이면 등록해도 다음 로그인 때 못 찾는다.
pub fn is_installed_location() -> bool {
    #[cfg(target_os = "macos")]
    {
        bundle_path().is_some_and(|path| !is_temporary_location(&path.to_string_lossy()))
    }
    #[cfg(not(target_os = "macos"))]
    {
        true
    }
}

// MARK: - macOS: 로그인 항목 (시스템 설정 → 일반 → 로그인 항목)

#[cfg(target_os = "macos")]
mod platform {
    use super::LaunchAtLoginState;
    use objc2::rc::Retained;
    use objc2_app_kit::NSRunningApplication;
    use objc2_foundation::{NSAppleEventDescriptor, NSAppleEventManager, NSString};
    use objc2_service_management::{SMAppService, SMAppServiceStatus};
    use std::time::Duration;
    use tauri::AppHandle;

    /// 사용자가 시스템 설정에서 직접 끌 수도 있으므로 상태는 저장해 두지 않고 매번 시스템에 묻는다.
    pub fn state(_app: &AppHandle) -> LaunchAtLoginState {
        // Safety: 인자 없는 클래스 메서드와 속성 읽기다.
        match unsafe { SMAppService::mainAppService().status() } {
            SMAppServiceStatus::Enabled => LaunchAtLoginState::Enabled,
            SMAppServiceStatus::RequiresApproval => LaunchAtLoginState::RequiresApproval,
            _ => LaunchAtLoginState::Disabled,
        }
    }

    pub fn set(_app: &AppHandle, enabled: bool) -> Result<(), String> {
        // Safety: 이 앱 자신을 로그인 항목으로 등록 · 해제한다.
        let result = unsafe {
            let service = SMAppService::mainAppService();
            if enabled {
                service.registerAndReturnError()
            } else {
                service.unregisterAndReturnError()
            }
        };
        result.map_err(|error| error.localizedDescription().to_string())
    }

    pub fn open_login_items_settings() {
        // Safety: 시스템 설정의 로그인 항목 화면을 연다.
        unsafe { SMAppService::openSystemSettingsLoginItems() };
    }

    const fn four_char_code(code: &[u8; 4]) -> u32 {
        u32::from_be_bytes(*code)
    }

    /// 로그인 항목으로 켜졌는지. macOS 가 실행 Apple Event 에 표시를 붙여 보낸다.
    /// 앱이 막 켜졌을 때(실행 이벤트를 처리하는 동안) 불러야 한다.
    pub fn is_login_item_launch() -> bool {
        const CORE_EVENT_CLASS: u32 = four_char_code(b"aevt");
        const OPEN_APPLICATION: u32 = four_char_code(b"oapp");
        const PROPERTY_DATA: u32 = four_char_code(b"prdt");
        const LAUNCHED_AS_LOGIN_ITEM: u32 = four_char_code(b"lgit");

        let Some(event) = NSAppleEventManager::sharedAppleEventManager().currentAppleEvent() else {
            return false;
        };
        // 이벤트 종류를 읽는 메서드들은 objc2-core-services 기능 뒤에 있어 직접 보낸다. 모두 FourCharCode(u32)를 주고받는다.
        // Safety: eventClass · eventID 는 인자 없이 u32 를, paramDescriptorForKeyword: 는 u32 를 받아 객체(또는 nil)를 돌려준다.
        unsafe {
            let class: u32 = objc2::msg_send![&*event, eventClass];
            let id: u32 = objc2::msg_send![&*event, eventID];
            if class != CORE_EVENT_CLASS || id != OPEN_APPLICATION {
                return false;
            }
            let property: Option<Retained<NSAppleEventDescriptor>> =
                objc2::msg_send![&*event, paramDescriptorForKeyword: PROPERTY_DATA];
            property.is_some_and(|property| property.enumCodeValue() == LAUNCHED_AS_LOGIN_ITEM)
        }
    }

    /// 이미 실행 중인 LinkGroupz 가 DMG 안(임시 위치)의 앱이고 지금 켠 쪽이 제대로 설치된 앱이면, 임시 쪽을 끝낸다.
    /// 그 밖의 중복 실행은 단일 실행 플러그인이 처리한다 (새로 켠 쪽이 끝나고, 실행 중인 쪽이 관리 창을 연다).
    pub fn replace_temporary_instance() {
        if !super::is_installed_location() {
            return;
        }
        let identifier = NSString::from_str(linkgroupz_core::brand::APP_IDENTIFIER);
        let me = std::process::id() as i32;
        let others: Vec<Retained<NSRunningApplication>> =
            NSRunningApplication::runningApplicationsWithBundleIdentifier(&identifier)
                .to_vec()
                .into_iter()
                .filter(|app| app.processIdentifier() != me && !app.isTerminated())
                .collect();
        for other in others {
            let location = other.bundleURL().and_then(|url| url.path()).map(|path| path.to_string()).unwrap_or_default();
            if !super::is_temporary_location(&location) {
                continue;
            }
            other.terminate();
            for _ in 0..30 {
                if other.isTerminated() {
                    break;
                }
                std::thread::sleep(Duration::from_millis(100));
            }
        }
    }
}

// MARK: - Windows: 시작 프로그램 (레지스트리 Run)

#[cfg(windows)]
mod platform {
    use super::LaunchAtLoginState;
    use tauri::AppHandle;
    use tauri_plugin_autostart::ManagerExt;

    pub fn state(app: &AppHandle) -> LaunchAtLoginState {
        match app.autolaunch().is_enabled() {
            Ok(true) => LaunchAtLoginState::Enabled,
            _ => LaunchAtLoginState::Disabled,
        }
    }

    pub fn set(app: &AppHandle, enabled: bool) -> Result<(), String> {
        let manager = app.autolaunch();
        let result = if enabled { manager.enable() } else { manager.disable() };
        result.map_err(|error| error.to_string())
    }

    pub fn open_login_items_settings() {}

    pub fn is_login_item_launch() -> bool {
        // 시작 프로그램으로 켜질 때는 `--autostart` 인자가 붙는다 (바깥의 is_login_item_launch 가 본다).
        false
    }

    pub fn replace_temporary_instance() {}
}

#[cfg(not(any(target_os = "macos", windows)))]
mod platform {
    use super::LaunchAtLoginState;
    use tauri::AppHandle;

    pub fn state(_app: &AppHandle) -> LaunchAtLoginState {
        LaunchAtLoginState::Disabled
    }

    pub fn set(_app: &AppHandle, _enabled: bool) -> Result<(), String> {
        Err(linkgroupz_core::tr!("이 운영체제에서는 지원하지 않습니다.", "Not supported on this operating system.").to_owned())
    }

    pub fn open_login_items_settings() {}

    pub fn is_login_item_launch() -> bool {
        false
    }

    pub fn replace_temporary_instance() {}
}

pub fn state(app: &AppHandle) -> LaunchAtLoginState {
    platform::state(app)
}

pub fn set(app: &AppHandle, enabled: bool) -> Result<(), String> {
    platform::set(app, enabled)
}

pub fn open_login_items_settings() {
    platform::open_login_items_settings();
}

pub fn is_login_item_launch() -> bool {
    std::env::args().any(|argument| argument == AUTOSTART_ARGUMENT) || platform::is_login_item_launch()
}

pub fn replace_temporary_instance() {
    platform::replace_temporary_instance();
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn temporary_locations() {
        assert!(is_temporary_location("/Volumes/LinkGroupz 1.5.4/LinkGroupz.app"), "DMG 를 연 볼륨");
        assert!(is_temporary_location("/private/var/folders/x/AppTranslocation/ABC/d/LinkGroupz.app"), "임시 위치에서 실행");
        assert!(!is_temporary_location("/Applications/LinkGroupz.app"), "응용 프로그램 폴더");
    }
}
