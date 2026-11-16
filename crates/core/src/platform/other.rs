//! macOS · Windows 가 아닌 곳(Linux 등)에서는 빌드와 검사만 되도록 최소한으로 둔다.

use crate::browser::Browser;
use crate::secret::SecretError;
use crate::timestamp::Timestamp;
use std::io;
use std::path::{Path, PathBuf};
use std::process::{Command, Stdio};

pub fn browser_user_data_directory(browser: Browser) -> PathBuf {
    let base = dirs::config_dir().unwrap_or_else(|| PathBuf::from("/"));
    match browser {
        Browser::Chrome => base.join("google-chrome"),
        Browser::Edge => base.join("microsoft-edge"),
        Browser::Whale => base.join("naver-whale"),
    }
}

pub fn browser_app_path(browser: Browser) -> Option<PathBuf> {
    let names: &[&str] = match browser {
        Browser::Chrome => &["google-chrome", "google-chrome-stable"],
        Browser::Edge => &["microsoft-edge", "microsoft-edge-stable"],
        Browser::Whale => &["naver-whale", "naver-whale-stable"],
    };
    let paths = std::env::var_os("PATH")?;
    std::env::split_paths(&paths)
        .flat_map(|folder| names.iter().map(move |name| folder.join(name)))
        .find(|path| path.is_file())
}

pub fn browser_window_numbers(_browser: Browser) -> std::collections::HashSet<u32> {
    Default::default()
}

pub fn frontmost_bundle_identifier() -> Option<String> {
    None
}

pub fn browser_policy_value(_browser: Browser, _name: &str) -> Option<i64> {
    None
}

pub fn browser_is_running(_browser: Browser) -> bool {
    false
}

pub fn browser_launch_date(_browser: Browser) -> Option<Timestamp> {
    None
}

pub fn app_icon_png(_app: &Path, _size: u32) -> Option<Vec<u8>> {
    None
}

fn spawn_detached(command: &mut Command) -> io::Result<()> {
    command.stdin(Stdio::null()).stdout(Stdio::null()).stderr(Stdio::null()).spawn().map(|_| ())
}

pub fn open_urls(app: &Path, urls: &[String]) -> io::Result<()> {
    spawn_detached(Command::new(app).args(urls))
}

pub fn launch_with_arguments(app: &Path, arguments: &[String]) -> io::Result<()> {
    spawn_detached(Command::new(app).args(arguments))
}

fn unsupported() -> SecretError {
    SecretError::Store(crate::tr!("이 운영체제에서는 비밀 값 보관을 지원하지 않습니다", "Storing secrets isn't supported on this operating system").to_owned())
}

pub fn secret_load(_service: &str, _account: &str) -> Result<Option<Vec<u8>>, SecretError> {
    Err(unsupported())
}

pub fn secret_save(_service: &str, _account: &str, _label: &str, _data: &[u8]) -> Result<(), SecretError> {
    Err(unsupported())
}

pub fn secret_delete(_service: &str, _account: &str) -> Result<(), SecretError> {
    Err(unsupported())
}
