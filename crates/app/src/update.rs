//! GitHub 릴리스에 올린 배포 파일로 업데이트한다 (Tauri 공식 업데이터).
//!
//! - 앱을 켠 뒤 30초, 그 뒤 1시간마다 새 버전을 확인한다 (설정에서 끌 수 있다).
//! - 새 버전이 있으면 '지금 업데이트 / 나중에' 창을 띄운다. '나중에'를 고른 버전은 더 새 버전이 올라오기 전까지 다시 묻지 않는다.
//! - 관리 창 오른쪽 아래 · 설정에서 직접 확인하고 업데이트할 수 있다.
//!
//! 배포 파일이 올라오는 저장소와 서명 공개 키는 소스에 없다. 빌드할 때 넣은 값이 없으면
//! 자동 업데이트는 쓸 수 없다 (`linkgroupz_core::UpdateSource`, crates/app/build.rs).
//! 내려받은 파일은 그 공개 키로 서명을 확인한 뒤에만 설치된다 — 서명에는 버전도 들어 있어,
//! 누가 latest.json 을 바꿔 치워도 예전 배포 파일을 새 버전인 것처럼 깔게 할 수 없다.

use crate::settings::Settings;
use crate::state::{lock, AppState};
use linkgroupz_core::{brand, AppVersion, Timestamp, UpdatePromptPolicy, UpdateSource};
use serde::Serialize;
use std::sync::Mutex;
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager};
use tauri_plugin_updater::{Update, UpdaterExt};

pub const CHANGED_EVENT: &str = "update-changed";
/// 자동으로 확인하는 간격
const CHECK_INTERVAL: Duration = Duration::from_secs(60 * 60);
/// 앱을 켠 뒤 처음 확인하기까지
const FIRST_CHECK_DELAY: Duration = Duration::from_secs(30);
/// 새 버전이 있는지 묻는 요청을 기다리는 시간
const CHECK_TIMEOUT: Duration = Duration::from_secs(30);
/// 배포 파일을 내려받는 요청을 기다리는 시간
const DOWNLOAD_TIMEOUT: Duration = Duration::from_secs(15 * 60);

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum Phase {
    Idle,
    Checking,
    Downloading,
    Installing,
}

#[derive(Debug, thiserror::Error)]
pub enum UpdateError {
    #[error("{}", linkgroupz_core::tr!("이 빌드에는 업데이트 주소가 들어 있지 않습니다.", "This build has no update address."))]
    NotConfigured,
    #[error("{}", linkgroupz_core::tr!("이 컴퓨터(운영체제 · CPU)용 배포 파일이 아직 올라오지 않았습니다.", "There is no release for this computer (OS · CPU) yet."))]
    NoReleaseForThisComputer,
    #[error("{}", linkgroupz_core::tr!("새 버전을 확인하지 못했습니다. 인터넷 연결을 확인해 주세요.", "Couldn't check for a new version. Check your internet connection."))]
    Unreachable,
    #[error("{}", linkgroupz_core::tr!("받은 파일의 서명이 맞지 않아 설치하지 않았습니다.", "The downloaded file's signature didn't match, so it wasn't installed."))]
    BadSignature,
    #[error("{}", linkgroupz_core::tr!("설치된 앱에서 실행 중이 아니라 업데이트할 수 없습니다. LinkGroupz 를 응용 프로그램 폴더로 옮긴 뒤 다시 해 주세요.", "Can't update because LinkGroupz isn't running from an installed location. Move it to the Applications folder and try again."))]
    #[cfg_attr(not(target_os = "macos"), allow(dead_code))]
    NotInstalled,
    #[error("{}", linkgroupz_core::tr!("업데이트하지 못했습니다. ({})", "Couldn't update. ({})", .0))]
    Other(String),
}

impl From<tauri_plugin_updater::Error> for UpdateError {
    fn from(error: tauri_plugin_updater::Error) -> Self {
        use tauri_plugin_updater::Error;
        match error {
            Error::TargetNotFound(_) | Error::TargetsNotFound(_) | Error::UnsupportedArch | Error::UnsupportedOs => {
                UpdateError::NoReleaseForThisComputer
            }
            Error::ReleaseNotFound | Error::Reqwest(_) | Error::Network(_) => UpdateError::Unreachable,
            Error::Minisign(_) | Error::SignedVersionMismatch { .. } | Error::MissingSignedVersion => UpdateError::BadSignature,
            other => UpdateError::Other(other.to_string()),
        }
    }
}

struct Inner {
    /// 1시간마다 스스로 확인할지
    auto_check: bool,
    /// 지금 버전보다 새 버전 (마지막으로 확인했을 때)
    available: Option<Update>,
    last_checked: Option<Timestamp>,
    phase: Phase,
    /// 직접 확인했을 때의 결과 한 줄 (최신 버전입니다 · 오류)
    message: Option<String>,
    timer: Option<tauri::async_runtime::JoinHandle<()>>,
    /// 내려받는 중인 버전과 받은 양 (진행 막대)
    installing_version: Option<String>,
    downloaded: u64,
    total: Option<u64>,
}

/// 화면에 보내는 업데이트 상태
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct UpdateStatus {
    /// 업데이트 주소가 빌드에 들어 있는지 (없으면 자동 업데이트를 쓸 수 없다)
    pub configured: bool,
    /// 1시간마다 스스로 확인하는지
    pub auto_check: bool,
    pub current_version: AppVersion,
    /// 지금 버전보다 높은 버전이 있으면 그 버전
    pub available_version: Option<String>,
    /// 마지막으로 확인한 시각 (유닉스 ms)
    pub last_checked: Option<f64>,
    pub phase: Phase,
    pub message: Option<String>,
    /// 새 버전의 바뀐 점 (릴리스의 latest.json notes)
    pub available_notes: Option<String>,
    /// 설치하는 중인 버전 (진행 창 제목)
    pub installing_version: Option<String>,
    /// 내려받은 바이트 · 전체 바이트 (모르면 None)
    pub downloaded: u64,
    pub total: Option<u64>,
}

pub struct UpdateController {
    source: Option<UpdateSource>,
    current_version: AppVersion,
    inner: Mutex<Inner>,
}

impl UpdateController {
    pub fn new(current_version: AppVersion, settings: &Settings) -> Self {
        // 빌드할 때 넣은 값 (crates/app/build.rs 가 update-source.env 나 환경 변수에서 읽는다)
        let source = UpdateSource::new(
            option_env!("LINKGROUPZ_BUILT_IN_RELEASES_REPO").unwrap_or_default(),
            option_env!("LINKGROUPZ_BUILT_IN_UPDATE_PUBKEY").unwrap_or_default(),
        );
        Self {
            source,
            current_version,
            inner: Mutex::new(Inner {
                auto_check: settings.auto_update_check,
                available: None,
                last_checked: None,
                phase: Phase::Idle,
                message: None,
                timer: None,
                installing_version: None,
                downloaded: 0,
                total: None,
            }),
        }
    }

    pub fn current_version(&self) -> &AppVersion {
        &self.current_version
    }

    pub fn source(&self) -> Option<&UpdateSource> {
        self.source.as_ref()
    }

    pub fn status(&self) -> UpdateStatus {
        let inner = lock(&self.inner);
        UpdateStatus {
            configured: self.source.is_some(),
            auto_check: inner.auto_check,
            current_version: self.current_version.clone(),
            available_version: inner.available.as_ref().map(|update| update.version.clone()),
            available_notes: inner.available.as_ref().and_then(|update| update.body.clone()),
            last_checked: inner.last_checked.map(|time| time.unix_seconds() * 1000.0),
            phase: inner.phase,
            message: inner.message.clone(),
            installing_version: inner.installing_version.clone(),
            downloaded: inner.downloaded,
            total: inner.total,
        }
    }

    fn change(&self, app: &AppHandle, change: impl FnOnce(&mut Inner)) {
        change(&mut lock(&self.inner));
        let _ = app.emit(CHANGED_EVENT, self.status());
    }

    /// 화면이 확인 결과 한 줄을 잠깐 보여 준 뒤 지운다.
    pub fn clear_message(&self, app: &AppHandle) {
        self.change(app, |inner| inner.message = None);
    }

    // MARK: - 시작 · 자동 확인 켜기 · 끄기

    /// 시험용: 확인할 주소(latest.json)를 환경 변수로 바꾼다. 서명은 그대로 앱에 든 공개 키로 확인한다.
    fn endpoint_override() -> Option<String> {
        std::env::var(brand::env::UPDATE_ENDPOINT).ok().filter(|endpoint| !endpoint.trim().is_empty())
    }

    /// 앱이 켜질 때 부른다. 예시 데이터로 띄운 앱은(확인할 주소를 환경 변수로 주지 않았으면) 확인하지 않는다.
    pub fn start(&self, app: &AppHandle) {
        let sample_data = std::env::var_os(brand::env::DATA_DIR).is_some();
        if sample_data && Self::endpoint_override().is_none() {
            return;
        }
        if self.source.is_some() && lock(&self.inner).auto_check {
            self.schedule_checks(app, Some(FIRST_CHECK_DELAY));
        }
    }

    pub fn set_auto_check(&self, app: &AppHandle, enabled: bool) {
        Self::update_settings(app, |settings| settings.auto_update_check = enabled);
        self.change(app, |inner| {
            inner.auto_check = enabled;
            if !enabled {
                if let Some(timer) = inner.timer.take() {
                    timer.abort();
                }
            }
        });
        if enabled && self.source.is_some() {
            // 켜자마자 한 번 확인하고, 그 뒤 1시간마다 확인한다.
            self.schedule_checks(app, Some(Duration::ZERO));
        }
    }

    fn update_settings(app: &AppHandle, change: impl FnOnce(&mut Settings)) {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        change(&mut settings);
        settings.save();
    }

    fn schedule_checks(&self, app: &AppHandle, first_delay: Option<Duration>) {
        let handle = app.clone();
        let timer = tauri::async_runtime::spawn(async move {
            let updater = handle.state::<AppState>().updater.clone();
            if let Some(delay) = first_delay {
                tokio::time::sleep(delay).await;
                updater.check(&handle, false).await;
            }
            loop {
                tokio::time::sleep(CHECK_INTERVAL).await;
                updater.check(&handle, false).await;
            }
        });
        if let Some(previous) = lock(&self.inner).timer.replace(timer) {
            previous.abort();
        }
    }

    // MARK: - 확인

    /// user_initiated: 사용자가 누른 확인이면 결과를 message 로 알리고 창은 띄우지 않는다 (업데이트 버튼이 보인다).
    pub async fn check(&self, app: &AppHandle, user_initiated: bool) {
        let Some(source) = self.source.clone() else {
            if user_initiated {
                self.change(app, |inner| inner.message = Some(UpdateError::NotConfigured.to_string()));
            }
            return;
        };
        {
            let mut inner = lock(&self.inner);
            if inner.phase != Phase::Idle {
                return;
            }
            inner.phase = Phase::Checking;
        }
        let _ = app.emit(CHANGED_EVENT, self.status());

        let result = Self::fetch_update(app, &source).await;
        let mut prompt = None;
        self.change(app, |inner| {
            inner.phase = Phase::Idle;
            match result {
                Ok(available) => {
                    inner.last_checked = Some(Timestamp::now());
                    if user_initiated {
                        inner.message = Some(match &available {
                            Some(update) => linkgroupz_core::tr!("새 버전 {} 이 있습니다.", "Version {} is available.", update.version),
                            None => linkgroupz_core::tr!("최신 버전입니다.", "You're up to date.").to_owned(),
                        });
                    } else {
                        prompt = available.as_ref().map(|update| update.version.clone());
                    }
                    inner.available = available;
                }
                Err(error) => {
                    if user_initiated {
                        inner.message = Some(error.to_string());
                    }
                }
            }
        });

        // 시험용(디버그 빌드에만 있다): 묻지 않고 바로 설치한다.
        #[cfg(debug_assertions)]
        if prompt.is_some() && std::env::var_os("LINKGROUPZ_UPDATE_AUTO_INSTALL").is_some() {
            return self.install(app).await;
        }

        if let Some(version) = prompt {
            let skipped = lock(&app.state::<AppState>().settings).update_skipped_version.clone();
            if self.should_prompt(&version, skipped.as_deref()) {
                self.prompt_for_update(app, version);
            }
        }
    }

    /// 릴리스의 latest.json 을 읽어, 지금 버전보다 새 버전이 있으면 돌려준다.
    async fn fetch_update(app: &AppHandle, source: &UpdateSource) -> Result<Option<Update>, UpdateError> {
        let endpoint = Self::endpoint_override().unwrap_or_else(|| source.manifest_url());
        let endpoint = url::Url::parse(endpoint.trim()).map_err(|error| UpdateError::Other(error.to_string()))?;
        let builder = app.updater_builder().timeout(CHECK_TIMEOUT).pubkey(source.public_key()).endpoints(vec![endpoint])?;
        Ok(builder.build()?.check().await?)
    }

    /// '나중에'를 고른 버전보다 높은 버전이 새로 올라오기 전까지는 다시 묻지 않는다.
    fn should_prompt(&self, version: &str, skipped: Option<&str>) -> bool {
        match AppVersion::parse(version) {
            Some(latest) => UpdatePromptPolicy::should_prompt(&latest, &self.current_version, skipped.and_then(AppVersion::parse).as_ref()),
            // 숫자만으로 된 버전이 아니면(1.6.0-beta.1 등) 같은 글자인지만 본다.
            None => skipped != Some(version),
        }
    }

    /// 새 버전이 있다고 알린다: 업데이트 창(update.html)에 버전 · 바뀐 점과 '나중에 · 지금 업데이트'를 보여 준다.
    /// '지금 업데이트'를 누르면 같은 창이 진행 막대로 바뀐다. (macOS 기본 알림 창은 모양을 바꿀 수 없어 쓰지 않는다.)
    fn prompt_for_update(&self, app: &AppHandle, _version: String) {
        crate::windows::show_update_progress(app);
    }

    /// 업데이트 창의 '나중에': 이 버전은 더 새 버전이 올라오기 전까지 다시 묻지 않는다.
    pub fn later(&self, app: &AppHandle) {
        let version = lock(&self.inner).available.as_ref().map(|update| update.version.clone());
        if let Some(version) = version {
            Self::update_settings(app, |settings| settings.update_skipped_version = Some(version));
        }
    }

    // MARK: - 설치

    /// 새 버전을 내려받아 설치하고 앱을 다시 켠다. 업데이트 창에 진행 막대를 보여 주고, 실패하면 그 창에 이유를 보여 준다.
    pub async fn install(&self, app: &AppHandle) {
        let update = {
            let mut inner = lock(&self.inner);
            if inner.phase != Phase::Idle {
                return;
            }
            let Some(update) = inner.available.clone() else {
                return;
            };
            inner.phase = Phase::Downloading;
            inner.message = None;
            inner.installing_version = Some(update.version.clone());
            inner.downloaded = 0;
            inner.total = None;
            update
        };
        let _ = app.emit(CHANGED_EVENT, self.status());
        crate::windows::show_update_progress(app);

        // 실패하면 진행 창에 이유와 '닫기'를 보여 준다 (설정의 업데이트 칸에도 남는다).
        if let Err(error) = self.download_and_install(app, update).await {
            let message = error.to_string();
            self.change(app, |inner| {
                inner.phase = Phase::Idle;
                inner.message = Some(message);
            });
            crate::windows::show_update_progress(app);
        }
    }

    async fn download_and_install(&self, app: &AppHandle, mut update: Update) -> Result<(), UpdateError> {
        // 앱 폴더(.app)를 통째로 바꿔 넣으므로 설치된 자리에서 실행 중이어야 한다 (DMG 안에서는 바꿀 수 없다).
        #[cfg(target_os = "macos")]
        let installed = crate::launch::bundle_path().filter(|_| crate::launch::is_installed_location()).ok_or(UpdateError::NotInstalled)?;

        update.timeout = Some(DOWNLOAD_TIMEOUT);
        // 서명 확인까지 플러그인이 한다. macOS 는 앱을 바꿔 넣고 돌아오고,
        // Windows 는 설치 프로그램을 창 없이(/S /R /UPDATE, tauri.conf.json 의 installMode: quiet) 띄운 뒤 이 앱을 끝내므로 돌아오지 않는다.
        // 받은 양은 진행 막대가 1% 움직일 때마다만 알린다.
        let mut downloaded: u64 = 0;
        let mut shown_percent: Option<u64> = None;
        update
            .download_and_install(
                |chunk, total| {
                    downloaded += chunk as u64;
                    let percent = total.filter(|total| *total > 0).map(|total| downloaded * 100 / total);
                    if percent != shown_percent || shown_percent.is_none() && downloaded % (1 << 20) < chunk as u64 {
                        shown_percent = percent;
                        self.change(app, |inner| {
                            inner.downloaded = downloaded;
                            inner.total = total;
                        });
                    }
                },
                || self.change(app, |inner| inner.phase = Phase::Installing),
            )
            .await?;

        #[cfg(target_os = "macos")]
        relaunch_after_exit(&installed)?;
        app.exit(0);
        Ok(())
    }
}

/// 이 앱이 끝나기를 기다렸다가 새로 넣은 앱을 켠다.
/// (끝나기 전에 켜면 '이미 실행 중'으로 보고 새로 켠 쪽이 바로 끝난다.)
#[cfg(target_os = "macos")]
fn relaunch_after_exit(app: &std::path::Path) -> Result<(), UpdateError> {
    use std::process::{Command, Stdio};

    // `open` 으로 켠 앱은 이 앱의 환경 변수를 물려받지 않는다. 예시 데이터 폴더 등 LinkGroupz 설정은 넘겨준다.
    let environment: Vec<String> = std::env::vars()
        .filter(|(name, _)| name.starts_with("LINKGROUPZ_"))
        .flat_map(|(name, value)| ["--env".to_owned(), format!("{name}={value}")])
        .collect();
    let script = r#"pid="$1"; app="$2"; shift 2; for _ in $(seq 1 200); do kill -0 "$pid" 2>/dev/null || break; sleep 0.1; done; open "$@" "$app""#;
    Command::new("/bin/sh")
        .args(["-c", script, "relaunch"])
        .arg(std::process::id().to_string())
        .arg(app)
        .args(environment)
        .stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null())
        .spawn()
        .map_err(|error| UpdateError::Other(error.to_string()))?;
    Ok(())
}
