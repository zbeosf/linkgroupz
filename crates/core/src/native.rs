use crate::brand;
use crate::browser::Browser;
use crate::extension::ChromeExtensionDetector;
use crate::fsutil::write_atomic;
use crate::snapshot::ChromeSnapshot;
use crate::store::{sorted_keys, LinkStore};
use crate::timestamp::Timestamp;
use serde_json::json;
use std::fs;
use std::io::{self, Read};
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
pub enum NativeSnapshotError {
    #[error("{}", crate::tr!("스냅샷이 아닌 메시지입니다: {}", "Not a snapshot message: {}", .0))]
    NotASnapshot(#[from] serde_json::Error),
    #[error("{0}")]
    Io(#[from] io::Error),
}

/// 확장이 탭 · 탭 그룹이 바뀔 때마다 Native Messaging 으로 보내 주는 스냅샷을 파일로 주고받는다.
///
/// 흐름: 확장 → (chrome.runtime.sendNativeMessage) → 호스트(앱과 함께 설치되는 작은 실행 파일) → {browser}-snapshot.json
/// 가져오기 화면은 이 파일만 읽으므로 브라우저에 탭을 열지 않는다.
/// Chrome · Edge · Whale 에 같은 확장을 넣으면 둘 다 같은 호스트를 부르므로, 확장이 보낸 browser 값으로 파일을 나눈다.
pub struct NativeSnapshotStore;

impl NativeSnapshotStore {
    pub const HOST_NAME: &'static str = brand::NATIVE_HOST_NAME;

    pub fn host_executable_name() -> String {
        if cfg!(windows) {
            format!("{}.exe", brand::HOST_EXECUTABLE_NAME)
        } else {
            brand::HOST_EXECUTABLE_NAME.to_owned()
        }
    }

    /// 확장이 자동 저장한 탭 목록 파일. 브라우저가 띄우는 호스트는 데이터 폴더 환경 변수를 모르므로
    /// 늘 실제 앱 데이터 폴더에 쓴다 (예시 데이터로 띄운 앱도 같은 파일을 읽는다).
    pub fn file_path(browser: Browser) -> PathBuf {
        LinkStore::default_data_directory().join(browser.snapshot_file_name())
    }

    /// 호스트 등록 파일 위치.
    ///
    /// macOS · Linux 는 브라우저가 사용자 데이터 폴더의 NativeMessagingHosts/ 에서 찾는다.
    /// Windows 는 레지스트리에 파일 위치를 적어 주므로 앱 데이터 폴더에 둔다.
    pub fn manifest_path(browser: Browser) -> PathBuf {
        if cfg!(windows) {
            LinkStore::default_data_directory().join("native-host").join(format!("{}.json", browser.raw_value()))
        } else {
            browser.user_data_directory().join("NativeMessagingHosts").join(format!("{}.json", Self::HOST_NAME))
        }
    }

    // MARK: - 호스트 쪽

    /// 받은 메시지가 스냅샷으로 읽히는지 확인한 뒤, 보낸 브라우저의 파일에 쓴다.
    pub fn write(message: &[u8], directory: &Path) -> Result<PathBuf, NativeSnapshotError> {
        let snapshot: ChromeSnapshot = serde_json::from_slice(message)?;
        let file = directory.join(snapshot.source_browser().snapshot_file_name());
        fs::create_dir_all(directory)?;
        write_atomic(&file, message)?;
        Ok(file)
    }

    // MARK: - 앱 쪽

    pub fn read(file: &Path) -> Option<ChromeSnapshot> {
        serde_json::from_slice(&fs::read(file).ok()?).ok()
    }

    /// 지금 실행 중인 브라우저가 켜진 뒤에 저장된 스냅샷만 쓴다. 그 전 것은 이미 닫힌 창 · 탭일 수 있다.
    pub fn read_fresh(file: &Path, browser_launch_date: Option<Timestamp>) -> Option<ChromeSnapshot> {
        let snapshot = Self::read(file)?;
        let saved_at = Timestamp::from_unix_millis(snapshot.saved_at?);
        if browser_launch_date.is_some_and(|launched| saved_at < launched) {
            return None;
        }
        Some(snapshot)
    }

    pub fn manifest_data(extension_id: &str, host_path: &str) -> Vec<u8> {
        let manifest = json!({
            "name": Self::HOST_NAME,
            "description": "LinkGroupz: 브라우저 탭 · 탭 그룹 목록을 앱에 전달",
            "path": host_path,
            "type": "stdio",
            "allowed_origins": [format!("chrome-extension://{extension_id}/")],
        });
        serde_json::to_vec_pretty(&sorted_keys(manifest)).expect("문자열만 든 값은 항상 JSON 으로 바뀐다")
    }

    /// 호스트 등록 파일을 쓴다. 내용이 같으면 쓰지 않는다. 바뀌었으면 true.
    pub fn install_manifest(extension_id: &str, host_path: &str, at: &Path) -> io::Result<bool> {
        let data = Self::manifest_data(extension_id, host_path);
        if fs::read(at).is_ok_and(|existing| existing == data) {
            return Ok(false);
        }
        if let Some(folder) = at.parent() {
            fs::create_dir_all(folder)?;
        }
        write_atomic(at, &data)?;
        Ok(true)
    }

    /// 앱과 함께 설치된 호스트 실행 파일.
    ///
    /// 개발 빌드(cargo run)와 앱 번들 밖에서 실행한 경우는 None — 빌드 폴더의 실행 파일을 브라우저에 등록하지 않는다.
    pub fn bundled_host_path() -> Option<PathBuf> {
        if cfg!(debug_assertions) {
            return None;
        }
        let executable = std::env::current_exe().ok()?;
        if cfg!(target_os = "macos") && !executable.to_string_lossy().contains(".app/Contents/MacOS/") {
            return None;
        }
        let host = executable.parent()?.join(Self::host_executable_name());
        host.is_file().then_some(host)
    }

    /// LinkGroupz 확장이 설치된 브라우저마다(설치된 앱에서 실행 중일 때) 호스트를 등록한다. 하나라도 새로 썼으면 true.
    ///
    /// 원본 LinkGroups 확장은 원본 앱의 호스트(다른 이름)를 부르므로 등록하지 않는다 — 원본 앱의 등록도 건드리지 않는다.
    pub fn install_if_possible() -> bool {
        let Some(host_path) = Self::bundled_host_path() else {
            return false;
        };
        let host_path = host_path.to_string_lossy();
        let mut changed = false;
        for browser in Browser::ALL {
            let Some(found) = ChromeExtensionDetector::find(browser) else {
                continue;
            };
            let manifest = Self::manifest_path(browser);
            if Self::install_manifest(&found.extension_id, &host_path, &manifest).unwrap_or(false) {
                changed = true;
            }
            #[cfg(windows)]
            let _ = Self::register_manifest(browser, &manifest);
        }
        changed
    }

    /// Windows 는 브라우저가 레지스트리에서 호스트 등록 파일의 위치를 찾는다.
    #[cfg(windows)]
    fn register_manifest(browser: Browser, manifest: &Path) -> io::Result<()> {
        use winreg::enums::HKEY_CURRENT_USER;
        use winreg::RegKey;

        let vendor = match browser {
            Browser::Chrome => "Google\\Chrome",
            Browser::Edge => "Microsoft\\Edge",
            Browser::Whale => "Naver\\Whale",
        };
        let subkey = format!("Software\\{vendor}\\NativeMessagingHosts\\{}", Self::HOST_NAME);
        let (key, _) = RegKey::predef(HKEY_CURRENT_USER).create_subkey(subkey)?;
        let path = manifest.to_string_lossy().into_owned();
        if key.get_value::<String, _>("").is_ok_and(|existing| existing == path) {
            return Ok(());
        }
        key.set_value("", &path)
    }
}

/// Chrome Native Messaging 형식: 4바이트 길이(호스트 바이트 순서 = little endian) + UTF-8 JSON
pub struct NativeMessaging;

impl NativeMessaging {
    pub const MAX_INCOMING_SIZE: usize = 64 * 1024 * 1024;

    /// 다음 메시지를 읽는다. 입력이 끝났거나 형식이 맞지 않으면 None.
    pub fn read_message(input: &mut impl Read) -> Option<Vec<u8>> {
        let mut header = [0u8; 4];
        input.read_exact(&mut header).ok()?;
        let length = u32::from_le_bytes(header) as usize;
        if length == 0 || length > Self::MAX_INCOMING_SIZE {
            return None;
        }
        let mut message = vec![0u8; length];
        input.read_exact(&mut message).ok()?;
        Some(message)
    }

    pub fn encode(message: &[u8]) -> Vec<u8> {
        let mut data = Vec::with_capacity(4 + message.len());
        data.extend_from_slice(&(message.len() as u32).to_le_bytes());
        data.extend_from_slice(message);
        data
    }
}
