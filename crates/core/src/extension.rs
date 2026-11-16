use crate::tr;
use crate::brand;
use crate::browser::Browser;
use crate::fsutil::copy_dir;
use crate::store::LinkStore;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use std::fs;
use std::path::{Path, PathBuf};

/// 그룹을 어떻게 열지. auto 는 LinkGroupz 확장이 설치돼 켜져 있으면 탭 그룹으로 연다.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum GroupOpenMode {
    #[default]
    Auto,
    TabGroup,
    NewWindow,
}

impl GroupOpenMode {
    pub const ALL: [GroupOpenMode; 3] = [GroupOpenMode::Auto, GroupOpenMode::TabGroup, GroupOpenMode::NewWindow];

    pub fn display_name(self) -> &'static str {
        match self {
            GroupOpenMode::Auto => tr!("자동 (확장이 있으면 탭 그룹)", "Auto (tab group with the extension)"),
            GroupOpenMode::TabGroup => tr!("항상 탭 그룹", "Always a tab group"),
            GroupOpenMode::NewWindow => tr!("항상 새 창", "Always a new window"),
        }
    }

    /// 확장 감지는 auto 일 때만 한다.
    /// '항상 탭 그룹'은 확장을 찾지 못해도 마커 주소로 연다 (확장이 없으면 접속할 수 없는 페이지가 열린다).
    pub fn uses_tab_group(self, extension_installed: impl FnOnce() -> bool) -> bool {
        match self {
            GroupOpenMode::Auto => extension_installed(),
            GroupOpenMode::TabGroup => true,
            GroupOpenMode::NewWindow => false,
        }
    }
}

/// 브라우저 프로필에서 찾은 LinkGroupz 확장
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct FoundExtension {
    pub profile: String,
    #[serde(rename = "extensionID")]
    pub extension_id: String,
    pub path: String,
}

/// 브라우저 프로필 설정 파일에서 LinkGroupz 확장이 설치돼 켜져 있는지 찾는다.
///
/// Chromium 은 압축해제된 확장을 `<프로필>/Secure Preferences`(또는 `Preferences`)의
/// `extensions.settings.<확장 ID>` 에 절대 경로와 함께 기록하고, 꺼진 확장에는 `disable_reasons` 를 남긴다.
/// 확장 ID 는 로드한 폴더 경로에 따라 달라진다.
///
/// 다른 확장의 폴더는 열지 않는다 (macOS 는 문서 · 데스크탑 · 다운로드 폴더 안을 읽으면 권한을 묻는다).
/// 1. 앱이 확장을 복사해 두는 곳(`BundledExtension::installed_dir`)에서 로드했으면 경로만 보고 안다.
/// 2. 다른 곳에서 로드했으면, 브라우저가 기록한 권한에 LinkGroupz 확장의 권한이 있고 권한을 묻는 폴더 밖인 것만
///    manifest.json 의 이름으로 확인한다.
pub struct ChromeExtensionDetector;

impl ChromeExtensionDetector {
    pub const MANIFEST_NAME: &'static str = brand::EXTENSION_MANIFEST_NAME;
    /// LinkGroupz 확장이 받는 권한 중 다른 확장에는 드문 것 (chrome-extension/manifest.json 의 permissions)
    const REQUIRED_PERMISSIONS: [&'static str; 2] = ["nativeMessaging", "tabGroups"];

    /// 그 브라우저(Chrome · Edge · Whale)에 LinkGroupz 확장이 설치돼 켜져 있으면 찾은 정보.
    pub fn find(browser: Browser) -> Option<FoundExtension> {
        Self::find_in(&browser.user_data_directory(), &BundledExtension::installed_dir())
    }

    /// installed_dir: 앱이 확장을 복사해 두는 곳. 프로필 순서대로 처음 찾은 것을 돌려준다.
    pub fn find_in(user_data_directory: &Path, installed_dir: &Path) -> Option<FoundExtension> {
        Self::profile_names(user_data_directory).into_iter().find_map(|profile| {
            ["Secure Preferences", "Preferences"].into_iter().find_map(|file_name| {
                Self::find_in_preferences_file(&user_data_directory.join(&profile).join(file_name), &profile, installed_dir)
            })
        })
    }

    /// 프로필 폴더 이름. Default 가 먼저, 그 뒤는 번호순 (Profile 2 < Profile 10)
    pub fn profile_names(user_data_directory: &Path) -> Vec<String> {
        let Ok(entries) = fs::read_dir(user_data_directory) else {
            return Vec::new();
        };
        let mut names: Vec<String> = entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name == "Default" || name.starts_with("Profile "))
            .collect();
        names.sort_by_cached_key(|name| {
            let number = name.strip_prefix("Profile ").and_then(|rest| rest.trim().parse::<u64>().ok());
            (name != "Default", number.unwrap_or(u64::MAX), name.clone())
        });
        names
    }

    /// 설정 파일에 기록된, 켜져 있는 확장들 (확장 ID 순)
    fn find_in_preferences_file(file: &Path, profile: &str, installed_dir: &Path) -> Option<FoundExtension> {
        let root: Value = serde_json::from_slice(&fs::read(file).ok()?).ok()?;
        let settings = root.get("extensions")?.get("settings")?.as_object()?;
        let mut ids: Vec<&String> = settings.keys().collect();
        ids.sort();
        ids.into_iter().find_map(|id| {
            let entry = &settings[id];
            let path = entry.get("path").and_then(Value::as_str)?;
            // 웹 스토어 확장은 상대 경로로 기록된다. 압축해제된 확장만 절대 경로다.
            if !Path::new(path).is_absolute() || !Self::is_enabled(entry) || !Self::is_ours(entry, Path::new(path), installed_dir) {
                return None;
            }
            Some(FoundExtension { profile: profile.to_owned(), extension_id: id.clone(), path: path.to_owned() })
        })
    }

    fn is_ours(entry: &Value, path: &Path, installed_dir: &Path) -> bool {
        if same_folder(path, installed_dir) {
            return true;
        }
        Self::has_required_permissions(entry)
            && !Self::is_permission_guarded(path)
            && Self::manifest_name(path).as_deref() == Some(Self::MANIFEST_NAME)
    }

    /// 브라우저가 기록해 둔 그 확장의 권한에 LinkGroupz 확장의 권한이 모두 있는지 (확장 폴더를 열지 않고 고른다)
    pub fn has_required_permissions(entry: &Value) -> bool {
        let api = |key: &str| {
            let list = entry.get(key)?.get("api")?.as_array()?;
            Some(list.iter().filter_map(Value::as_str).collect::<Vec<_>>())
        };
        let Some(api) = api("active_permissions").or_else(|| api("granted_permissions")) else {
            return false;
        };
        Self::REQUIRED_PERMISSIONS.iter().all(|permission| api.contains(permission))
    }

    /// macOS 가 읽기 전에 사용자에게 권한을 묻는 곳인지 (문서 · 데스크탑 · 다운로드 · iCloud Drive · 클라우드 저장소 · 외장 디스크).
    /// 다른 운영체제는 묻지 않는다.
    pub fn is_permission_guarded(path: &Path) -> bool {
        if !cfg!(target_os = "macos") {
            return false;
        }
        let Some(home) = dirs::home_dir() else {
            return true;
        };
        let guarded = [
            home.join("Documents"),
            home.join("Desktop"),
            home.join("Downloads"),
            home.join("Library/Mobile Documents"),
            home.join("Library/CloudStorage"),
            PathBuf::from("/Volumes"),
        ];
        guarded.iter().any(|folder| path.starts_with(folder))
    }

    pub fn is_enabled(entry: &Value) -> bool {
        match entry.get("disable_reasons") {
            Some(Value::Array(reasons)) => return reasons.is_empty(),
            Some(Value::Number(reasons)) => return reasons.as_i64() == Some(0),
            _ => {}
        }
        // 예전 Chrome 은 state 로 기록했다 (1: 켜짐, 0: 꺼짐)
        if let Some(state) = entry.get("state").and_then(Value::as_i64) {
            return state == 1;
        }
        true
    }

    fn manifest_name(extension_path: &Path) -> Option<String> {
        let root: Value = serde_json::from_slice(&fs::read(extension_path.join("manifest.json")).ok()?).ok()?;
        root.get("name")?.as_str().map(str::to_owned)
    }
}

/// 같은 폴더인지. Windows 는 대소문자 · 구분자(`\\` · `/`)를 가리지 않는다.
fn same_folder(a: &Path, b: &Path) -> bool {
    if cfg!(windows) {
        let text = |path: &Path| path.to_string_lossy().replace('/', "\\").trim_end_matches('\\').to_lowercase();
        text(a) == text(b)
    } else {
        a == b
    }
}

/// 앱 안에 들어 있는 브라우저 확장을 사용자 데이터 폴더로 복사해 둔다.
///
/// 브라우저는 "압축해제된 확장 프로그램"을 로드한 폴더를 계속 읽는다. 앱 안의 폴더를 로드하면
/// 앱을 지우거나 옮길 때 확장이 깨지므로, 앱이 바뀌어도 그대로인 위치에 두고 그 폴더를 로드하게 안내한다.
/// 앱을 업데이트해 확장 버전이 바뀌면 다시 복사한다 (브라우저에서 확장 새로고침 필요).
pub struct BundledExtension;

impl BundledExtension {
    /// 예시 데이터로 실행해도(데이터 폴더 환경 변수) 확장은 항상 실제 앱 데이터 폴더에 둔다.
    pub fn installed_dir() -> PathBuf {
        LinkStore::default_data_directory().join("chrome-extension")
    }

    pub fn version(folder: &Path) -> Option<String> {
        let manifest: Value = serde_json::from_slice(&fs::read(folder.join("manifest.json")).ok()?).ok()?;
        manifest.get("version")?.as_str().map(str::to_owned)
    }

    /// 복사했거나 이미 같은 버전이면 설치 위치를, 앱에 확장이 없으면 None 을 돌려준다.
    pub fn sync(source: Option<&Path>, target: &Path) -> Option<PathBuf> {
        let source = source?;
        let source_version = Self::version(source)?;
        if Self::version(target).as_deref() == Some(source_version.as_str()) {
            return Some(target.to_owned());
        }
        if let Some(parent) = target.parent() {
            fs::create_dir_all(parent).ok()?;
        }
        if target.exists() {
            fs::remove_dir_all(target).ok()?;
        }
        copy_dir(source, target).ok()?;
        Some(target.to_owned())
    }
}
