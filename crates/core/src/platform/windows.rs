use crate::browser::Browser;
use crate::secret::SecretError;
use crate::timestamp::Timestamp;
use image::RgbaImage;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};
use sysinfo::{ProcessRefreshKind, ProcessesToUpdate, System};
use winreg::enums::{HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE};
use winreg::RegKey;

// MARK: - 브라우저

pub fn browser_user_data_directory(browser: Browser) -> PathBuf {
    let base = dirs::data_local_dir().unwrap_or_else(|| PathBuf::from("C:\\"));
    match browser {
        Browser::Chrome => base.join("Google").join("Chrome").join("User Data"),
        Browser::Edge => base.join("Microsoft").join("Edge").join("User Data"),
        Browser::Whale => base.join("Naver").join("Naver Whale").join("User Data"),
    }
}

/// 레지스트리의 App Paths(설치 프로그램이 등록)에서 찾고, 없으면 잘 알려진 설치 위치를 본다.
pub fn browser_app_path(browser: Browser) -> Option<PathBuf> {
    let executable = browser.windows_executable();
    let subkey = format!("SOFTWARE\\Microsoft\\Windows\\CurrentVersion\\App Paths\\{executable}");
    for root in [HKEY_CURRENT_USER, HKEY_LOCAL_MACHINE] {
        let Ok(key) = RegKey::predef(root).open_subkey(&subkey) else {
            continue;
        };
        let Ok(value) = key.get_value::<String, _>("") else {
            continue;
        };
        let path = PathBuf::from(value.trim().trim_matches('"'));
        if path.is_file() {
            return Some(path);
        }
    }

    let relative = match browser {
        Browser::Chrome => Path::new("Google").join("Chrome").join("Application").join(executable),
        Browser::Edge => Path::new("Microsoft").join("Edge").join("Application").join(executable),
        Browser::Whale => Path::new("Naver").join("Naver Whale").join("Application").join(executable),
    };
    ["ProgramFiles", "ProgramFiles(x86)", "LOCALAPPDATA"]
        .into_iter()
        .filter_map(std::env::var_os)
        .map(|base| PathBuf::from(base).join(&relative))
        .find(|path| path.is_file())
}

/// 관리 정책 값 (DWORD). 컴퓨터 정책(HKLM)이 사용자 정책(HKCU)보다 앞선다.
pub fn browser_policy_value(browser: Browser, name: &str) -> Option<i64> {
    let subkey = match browser {
        Browser::Chrome => "SOFTWARE\\Policies\\Google\\Chrome",
        Browser::Edge => "SOFTWARE\\Policies\\Microsoft\\Edge",
        Browser::Whale => "SOFTWARE\\Policies\\Naver\\Whale",
    };
    [HKEY_LOCAL_MACHINE, HKEY_CURRENT_USER].into_iter().find_map(|root| {
        let key = RegKey::predef(root).open_subkey(subkey).ok()?;
        key.get_value::<u32, _>(name).ok().map(i64::from)
    })
}

/// (macOS 전용) 브라우저 창 번호. Windows 에서는 쓰지 않는다.
pub fn browser_window_numbers(_browser: Browser) -> std::collections::HashSet<u32> {
    Default::default()
}

pub fn frontmost_bundle_identifier() -> Option<String> {
    None
}

/// 실행 중인 브라우저 프로세스들이 켜진 시각(유닉스 초). 프로세스가 없으면 None.
fn browser_process_start_times(browser: Browser) -> Option<Vec<u64>> {
    let mut system = System::new();
    system.refresh_processes_specifics(ProcessesToUpdate::All, true, ProcessRefreshKind::nothing());
    let executable = browser.windows_executable();
    let times: Vec<u64> = system
        .processes()
        .values()
        .filter(|process| process.name().to_string_lossy().eq_ignore_ascii_case(executable))
        .map(|process| process.start_time())
        .collect();
    if times.is_empty() {
        None
    } else {
        Some(times)
    }
}

pub fn browser_is_running(browser: Browser) -> bool {
    browser_process_start_times(browser).is_some()
}

pub fn browser_launch_date(browser: Browser) -> Option<Timestamp> {
    // 브라우저는 프로세스가 여럿이다. 가장 먼저 켜진 것이 브라우저를 켠 시각이다.
    let earliest = browser_process_start_times(browser)?.into_iter().filter(|time| *time > 0).min()?;
    Some(Timestamp::from_unix_seconds(earliest as f64))
}

/// 실행 파일의 아이콘을 PNG 로 만든다.
pub fn app_icon_png(app: &Path, size: u32) -> Option<Vec<u8>> {
    let icon = file_icon_provider::get_file_icon(app, u16::try_from(size).ok()?).ok()?;
    let image = RgbaImage::from_raw(icon.width, icon.height, icon.pixels)?;
    super::icon_to_png(image, size)
}

// MARK: - 실행

fn spawn_detached(command: &mut Command) -> io::Result<()> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| ())
}

/// 주소를 그 브라우저로 연다 (이미 실행 중이면 그 브라우저에 넘긴다).
pub fn open_urls(app: &Path, urls: &[String]) -> io::Result<()> {
    spawn_detached(Command::new(app).args(urls))
}

/// 실행 인자를 붙여 브라우저를 띄운다. 실행 중인 브라우저에도 인자가 전달된다.
pub fn launch_with_arguments(app: &Path, arguments: &[String]) -> io::Result<()> {
    spawn_detached(Command::new(app).args(arguments))
}

// MARK: - 비밀 값 (Windows 자격 증명 관리자)

fn secret_error(error: keyring::Error) -> SecretError {
    SecretError::Store(error.to_string())
}

fn secret_entry(service: &str, account: &str) -> Result<keyring::Entry, SecretError> {
    keyring::Entry::new(service, account).map_err(secret_error)
}

pub fn secret_load(service: &str, account: &str) -> Result<Option<Vec<u8>>, SecretError> {
    match secret_entry(service, account)?.get_secret() {
        Ok(data) => Ok(Some(data)),
        Err(keyring::Error::NoEntry) => Ok(None),
        Err(error) => Err(secret_error(error)),
    }
}

pub fn secret_save(service: &str, account: &str, _label: &str, data: &[u8]) -> Result<(), SecretError> {
    secret_entry(service, account)?.set_secret(data).map_err(secret_error)
}

pub fn secret_delete(service: &str, account: &str) -> Result<(), SecretError> {
    match secret_entry(service, account)?.delete_credential() {
        Ok(()) | Err(keyring::Error::NoEntry) => Ok(()),
        Err(error) => Err(secret_error(error)),
    }
}
