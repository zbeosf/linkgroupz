//! 페이지 미리보기 창.
//!
//! 창 하나에 웹뷰 둘을 넣는다: 위쪽 띠(제목 · 주소 · 버튼, 앱의 화면)와 그 아래 웹 페이지.
//! 웹 페이지 쪽은 앱의 기능을 부를 수 없다 (앱 화면이 아닌 주소에는 권한을 주지 않는다).
//!
//! - 작은 미리보기: 페이지 줄의 눈 버튼으로 줄 옆에 띄운다. 0.7 배로 줄여 보여 주고, 다른 곳을 누르면 닫힌다.
//! - 크게 열기: 원래 크기 창. 로그인이 필요하면 여기서 한다.
//!
//! Chrome 과 쿠키를 공유하지 않는 별도 브라우저라, 로그인이 필요한 페이지는 처음에 로그인 화면이 보인다.
//! 한 번 로그인하면 쿠키를 보관해 두고 다음부터 쓴다.
//! - macOS: 디스크에 남기지 않는 저장소 + 키체인 (`preview_cookies`, `CookieVault`)
//! - Windows: WebView2 프로필 폴더 (WebView2 가 직접 암호화해 보관)

use crate::state::{lock, AppState};
use crate::windows;
use linkgroupz_core::{Browser, Id, LinkStore};
use serde::{Deserialize, Serialize};
use std::collections::HashMap;
use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Mutex;
use std::time::{Duration, Instant};
use tauri::webview::{NewWindowResponse, PageLoadEvent, WebviewBuilder};
use tauri::window::WindowBuilder;
use tauri::{
    AppHandle, Emitter, LogicalPosition, LogicalSize, Manager, PhysicalPosition, Url, WebviewUrl, Window, WindowEvent,
};

/// 작은 미리보기 창의 이름 (한 번에 하나만 뜬다)
pub const COMPACT: &str = "preview";
/// 크게 연 창 이름의 머리 (`browser-1`, `browser-2` …)
const EXPANDED_PREFIX: &str = "browser-";

const INFO_EVENT: &str = "preview-info";
const CLOSED_EVENT: &str = "preview-closed";

/// 작은 미리보기: 웹 페이지 영역 640×420 (0.7 배)
const COMPACT_SIZE: (f64, f64) = (640.0, 420.0);
/// 작은 미리보기를 줄일 수 있는 가장 작은 크기 (띠 제외)
const COMPACT_MIN_SIZE: (f64, f64) = (420.0, 240.0);
const COMPACT_ZOOM: f64 = 0.7;
const COMPACT_BAR: f64 = 88.0;
const EXPANDED_SIZE: (f64, f64) = (1100.0, 780.0);
const EXPANDED_BAR: f64 = 46.0;
/// 이 시간이 지나도 다 불러오지 못하면 느리다고 알린다.
const SLOW_AFTER: Duration = Duration::from_secs(20);
/// 작은 미리보기가 포커스를 잃어 닫힌 직후, 같은 페이지의 눈 버튼 클릭은 '닫기'로 본다.
const REOPEN_GUARD: Duration = Duration::from_millis(350);

/// 위쪽 띠가 보여 주는 내용
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PreviewInfo {
    pub label: String,
    pub compact: bool,
    /// 페이지 제목 (아직 모르면 빈 문자열)
    pub title: String,
    /// 제목을 모를 때 보여 줄 이름
    pub fallback_title: String,
    pub url: String,
    pub loading: bool,
    /// 오래 걸리고 있다
    pub slow: bool,
    pub browser: Browser,
    pub is_private: bool,
    #[serde(rename = "linkID")]
    pub link_id: Option<Id>,
    /// 몇 번째 탐색인지 (느림 알림이 지난 탐색의 것인지 가릴 때 쓴다)
    #[serde(skip)]
    generation: u64,
}

/// 관리 창이 알려 주는, 눌린 페이지 줄의 위치 (창 안쪽 기준, CSS 픽셀)
#[derive(Clone, Copy, Debug, Deserialize)]
pub struct Anchor {
    pub left: f64,
    pub top: f64,
    pub right: f64,
    pub bottom: f64,
}

pub struct PreviewRequest {
    pub url: String,
    pub fallback_title: String,
    pub browser: Browser,
    pub is_private: bool,
    pub link_id: Option<Id>,
    pub compact: bool,
    pub anchor: Option<Anchor>,
}

pub struct PreviewSessions {
    infos: Mutex<HashMap<String, PreviewInfo>>,
    counter: AtomicU64,
    /// 작은 미리보기가 포커스를 잃어 닫힌 시각과 보여 주던 페이지
    compact_closed: Mutex<Option<(Instant, Option<Id>)>>,
    /// 사용자가 창 크기를 바꿀 때마다 올린다. 끄기를 멈춘 뒤에 한 번만 설정 파일에 쓴다.
    resize_generation: AtomicU64,
    #[cfg(target_os = "macos")]
    vault: linkgroupz_core::CookieVault,
    /// 키체인의 쿠키를 저장소에 넣었는지 (앱을 켠 뒤 처음 미리보기를 열 때 한 번)
    #[cfg(target_os = "macos")]
    restored: Mutex<bool>,
}

impl PreviewSessions {
    pub fn new() -> Self {
        Self {
            infos: Mutex::new(HashMap::new()),
            counter: AtomicU64::new(0),
            compact_closed: Mutex::new(None),
            resize_generation: AtomicU64::new(0),
            #[cfg(target_os = "macos")]
            vault: linkgroupz_core::CookieVault::default(),
            #[cfg(target_os = "macos")]
            restored: Mutex::new(false),
        }
    }

    pub fn info(&self, label: &str) -> Option<PreviewInfo> {
        lock(&self.infos).get(label).cloned()
    }

    /// 작은 미리보기가 보여 주는 페이지
    pub fn compact_link(&self) -> Option<Id> {
        self.info(COMPACT).and_then(|info| info.link_id)
    }

    fn update(&self, app: &AppHandle, label: &str, change: impl FnOnce(&mut PreviewInfo)) {
        let updated = {
            let mut infos = lock(&self.infos);
            let Some(info) = infos.get_mut(label) else {
                return;
            };
            change(info);
            info.clone()
        };
        let _ = app.emit(INFO_EVENT, updated);
    }
}

fn sessions(app: &AppHandle) -> &PreviewSessions {
    &app.state::<AppState>().inner().previews
}

fn bar_label(label: &str) -> String {
    format!("{label}-bar")
}

fn content_label(label: &str) -> String {
    format!("{label}-web")
}

/// 띠 웹뷰의 이름에서 창 이름을 얻는다.
pub fn window_label_of_bar(bar: &str) -> Option<&str> {
    bar.strip_suffix("-bar")
}

fn bar_height(compact: bool) -> f64 {
    if compact {
        COMPACT_BAR
    } else {
        EXPANDED_BAR
    }
}

/// Windows 에서 미리보기가 쓰는 WebView2 프로필 폴더
#[cfg_attr(target_os = "macos", allow(dead_code))]
fn profile_directory() -> PathBuf {
    LinkStore::data_directory().join("preview-profile")
}

// MARK: - 열기 · 닫기

/// 미리보기를 연다. 기다리는 일이 있어 백그라운드 스레드에서 불러야 한다.
/// 작은 미리보기를 방금 닫은 그 페이지로 다시 열라는 요청이면 열지 않고 false 를 돌려준다.
pub fn open(app: &AppHandle, request: PreviewRequest) -> Result<bool, String> {
    let url = Url::parse(&request.url).map_err(|_| linkgroupz_core::tr!("미리보기로 열 수 없는 주소입니다.", "This address can't be previewed.").to_owned())?;
    let state = sessions(app);

    if request.compact {
        let guard = *lock(&state.compact_closed);
        if guard.is_some_and(|(at, link)| at.elapsed() < REOPEN_GUARD && link == request.link_id && link.is_some()) {
            return Ok(false);
        }
        // 다른 페이지를 보여 주던 작은 미리보기는 닫고 새로 연다.
        close(app, COMPACT);
    }

    prepare_cookies(app);

    let label = if request.compact {
        COMPACT.to_owned()
    } else {
        format!("{EXPANDED_PREFIX}{}", state.counter.fetch_add(1, Ordering::Relaxed) + 1)
    };
    let info = PreviewInfo {
        label: label.clone(),
        compact: request.compact,
        title: String::new(),
        fallback_title: request.fallback_title.clone(),
        url: request.url.clone(),
        loading: true,
        slow: false,
        browser: request.browser,
        is_private: request.is_private,
        link_id: request.link_id,
        generation: 0,
    };
    lock(&state.infos).insert(label.clone(), info);

    let built = build_on_ui_thread(app, label.clone(), url, request.compact, request.anchor);
    if let Err(error) = &built {
        lock(&state.infos).remove(&label);
        if let Some(window) = app.get_window(&label) {
            let _ = window.destroy();
        }
        return Err(linkgroupz_core::tr!("미리보기 창을 만들지 못했습니다: {}", "Couldn't create the preview window: {}", error));
    }
    Ok(true)
}

/// macOS 는 WebKit 설정을 메인 스레드에서 만들어야 하고, Windows 는 메인 스레드에서 웹뷰를 만들면 멈춘다 (WebView2).
#[cfg(target_os = "macos")]
fn build_on_ui_thread(app: &AppHandle, label: String, url: Url, compact: bool, anchor: Option<Anchor>) -> tauri::Result<()> {
    let (sender, receiver) = std::sync::mpsc::channel();
    let handle = app.clone();
    app.run_on_main_thread(move || {
        let _ = sender.send(build(&handle, &label, url, compact, anchor));
    })?;
    receiver.recv().unwrap_or(Err(tauri::Error::FailedToReceiveMessage))
}

#[cfg(not(target_os = "macos"))]
fn build_on_ui_thread(app: &AppHandle, label: String, url: Url, compact: bool, anchor: Option<Anchor>) -> tauri::Result<()> {
    build(app, &label, url, compact, anchor)
}

fn build(app: &AppHandle, label: &str, url: Url, compact: bool, anchor: Option<Anchor>) -> tauri::Result<()> {
    let bar = bar_height(compact);
    let (width, height) = window_size(app, compact);

    let mut builder = WindowBuilder::new(app, label).title(linkgroupz_core::tr!("미리보기", "Preview")).inner_size(width, height).visible(false);
    if compact {
        // 가장자리를 끌어 크기를 바꿀 수 있다 (바꾼 크기로 계속 연다).
        builder = builder
            .resizable(true)
            .min_inner_size(COMPACT_MIN_SIZE.0, COMPACT_MIN_SIZE.1 + bar)
            .decorations(false)
            .skip_taskbar(true)
            .shadow(true);
        if let Some(manager) = app.get_window(windows::MANAGER) {
            builder = builder.parent(&manager)?;
        }
    } else {
        builder = builder.min_inner_size(700.0, 500.0);
    }
    let window = builder.build()?;

    window.add_child(
        WebviewBuilder::new(bar_label(label), WebviewUrl::App("preview.html".into())),
        LogicalPosition::new(0.0, 0.0),
        LogicalSize::new(width, bar),
    )?;

    let content = WebviewBuilder::new(content_label(label), WebviewUrl::External(url))
        .on_page_load({
            let label = label.to_owned();
            move |webview, payload| {
                let app = webview.app_handle().clone();
                let url = payload.url().to_string();
                match payload.event() {
                    PageLoadEvent::Started => page_started(&app, &label, url),
                    PageLoadEvent::Finished => page_finished(&app, &label, url),
                }
            }
        })
        .on_document_title_changed({
            let label = label.to_owned();
            move |webview, title| {
                let app = webview.app_handle().clone();
                sessions(&app).update(&app, &label, |info| info.title = title.clone());
                if !compact && !title.is_empty() {
                    let _ = webview.window().set_title(&title);
                }
            }
        })
        // SSO 로그인 버튼처럼 새 창(target=_blank, window.open)을 여는 요청은 같은 창에서 연다.
        // 처리하지 않으면 버튼을 눌러도 아무 일도 일어나지 않는다.
        .on_new_window({
            let app = app.clone();
            let content = content_label(label);
            move |url, _features| {
                if let Some(webview) = app.get_webview(&content) {
                    let _ = webview.navigate(url);
                }
                NewWindowResponse::Deny
            }
        });
    #[cfg(target_os = "macos")]
    let content = match objc2::MainThreadMarker::new() {
        Some(mtm) => content.with_webview_configuration(crate::preview_cookies::configuration(mtm)),
        // 메인 스레드가 아니면 공유 저장소를 쓸 수 없다. 이 창에만 쓰는, 디스크에 남기지 않는 저장소로 연다.
        None => content.incognito(true),
    };
    #[cfg(not(target_os = "macos"))]
    let content = content.data_directory(profile_directory());

    let content = window.add_child(content, LogicalPosition::new(0.0, bar), LogicalSize::new(width, height - bar))?;
    if compact {
        let _ = content.set_zoom(COMPACT_ZOOM);
    }

    if let (true, Some(anchor)) = (compact, anchor) {
        place_compact(app, &window, anchor);
    }

    let app = app.clone();
    let label = label.to_owned();
    let watched = window.clone();
    window.on_window_event(move |event| match event {
        WindowEvent::Resized(_) | WindowEvent::ScaleFactorChanged { .. } => {
            layout(&watched, compact);
            if watched.is_visible().unwrap_or(false) {
                remember_size(&app, &watched, compact);
            }
        }
        WindowEvent::Focused(false) if compact => {
            let link = sessions(&app).compact_link();
            *lock(&sessions(&app).compact_closed) = Some((Instant::now(), link));
            let _ = watched.close();
        }
        WindowEvent::Destroyed => closed(&app, &label),
        _ => {}
    });

    let _ = window.show();
    let _ = window.set_focus();
    Ok(())
}

/// 미리보기 창 크기: 사용자가 바꿔 둔 크기, 없으면 기본 크기 (작은 미리보기는 띠 높이 포함)
fn window_size(app: &AppHandle, compact: bool) -> (f64, f64) {
    let state = app.state::<AppState>();
    let settings = lock(&state.settings);
    let chosen = if compact { settings.preview_size } else { settings.browser_size };
    chosen.unwrap_or(window_size_default(compact))
}

/// 사용자가 바꾼 창 크기를 기억한다. 끄는 동안 여러 번 오므로 설정 파일은 멈춘 뒤 한 번 쓴다.
fn remember_size(app: &AppHandle, window: &Window, compact: bool) {
    let (Ok(scale), Ok(size)) = (window.scale_factor(), window.inner_size()) else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    let size = Some((size.width.round(), size.height.round()));
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        let slot = if compact { &mut settings.preview_size } else { &mut settings.browser_size };
        if *slot == size || (slot.is_none() && size == Some(window_size_default(compact))) {
            return;
        }
        *slot = size;
    }
    let generation = sessions(app).resize_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        if sessions(&app).resize_generation.load(Ordering::SeqCst) == generation {
            lock(&app.state::<AppState>().settings).save();
        }
    });
}

fn window_size_default(compact: bool) -> (f64, f64) {
    if compact {
        (COMPACT_SIZE.0, COMPACT_SIZE.1 + bar_height(true))
    } else {
        EXPANDED_SIZE
    }
}

/// 창 크기가 바뀌면 띠는 위에 그대로 두고 웹 페이지가 나머지를 채운다.
fn layout(window: &Window, compact: bool) {
    let (Ok(scale), Ok(size)) = (window.scale_factor(), window.inner_size()) else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    let bar = bar_height(compact);
    let app = window.app_handle();
    if let Some(webview) = app.get_webview(&bar_label(window.label())) {
        let _ = webview.set_position(LogicalPosition::new(0.0, 0.0));
        let _ = webview.set_size(LogicalSize::new(size.width, bar));
    }
    if let Some(webview) = app.get_webview(&content_label(window.label())) {
        let _ = webview.set_position(LogicalPosition::new(0.0, bar));
        let _ = webview.set_size(LogicalSize::new(size.width, (size.height - bar).max(0.0)));
    }
}

/// 작은 미리보기를 눌린 페이지 줄 아래(자리가 없으면 위)에, 줄의 오른쪽 끝에 맞춰 놓는다.
fn place_compact(app: &AppHandle, window: &Window, anchor: Anchor) {
    let Some(manager) = app.get_window(windows::MANAGER) else {
        return;
    };
    let (Ok(scale), Ok(origin), Ok(size)) = (manager.scale_factor(), manager.inner_position(), window.outer_size()) else {
        return;
    };
    let (width, height) = (f64::from(size.width), f64::from(size.height));
    let gap = 6.0 * scale;
    let (row_right, row_top, row_bottom) = (
        f64::from(origin.x) + anchor.right * scale,
        f64::from(origin.y) + anchor.top * scale,
        f64::from(origin.y) + anchor.bottom * scale,
    );
    let mut x = row_right - width - 160.0 * scale;
    let mut y = row_bottom + gap;
    if let Ok(Some(monitor)) = app.monitor_from_point(row_right, row_top) {
        let area = monitor.work_area();
        let (left, top) = (f64::from(area.position.x), f64::from(area.position.y));
        let (right, bottom) = (left + f64::from(area.size.width), top + f64::from(area.size.height));
        if y + height > bottom && row_top - gap - height >= top {
            y = row_top - gap - height;
        }
        x = x.clamp(left + gap, (right - width - gap).max(left + gap));
        y = y.clamp(top, (bottom - height).max(top));
    }
    x = x.max(f64::from(origin.x) + anchor.left * scale + gap).min(row_right - width);
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

pub fn close(app: &AppHandle, label: &str) {
    if let Some(window) = app.get_window(label) {
        let _ = window.close();
    }
}

fn closed(app: &AppHandle, label: &str) {
    let removed = lock(&sessions(app).infos).remove(label);
    if let Some(info) = removed {
        let _ = app.emit(CLOSED_EVENT, info);
    }
}

// MARK: - 페이지 이동

fn page_started(app: &AppHandle, label: &str, url: String) {
    let mut generation = 0;
    sessions(app).update(app, label, |info| {
        info.generation += 1;
        generation = info.generation;
        info.loading = true;
        info.slow = false;
        info.url = url;
    });
    let app = app.clone();
    let label = label.to_owned();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(SLOW_AFTER).await;
        sessions(&app).update(&app, &label, |info| {
            if info.loading && info.generation == generation {
                info.slow = true;
            }
        });
    });
}

fn page_finished(app: &AppHandle, label: &str, url: String) {
    sessions(app).update(app, label, |info| {
        info.loading = false;
        info.slow = false;
        info.url = url;
    });
    // 로그인하면 쿠키가 바뀌므로 페이지를 다 불러올 때마다 키체인에 맞춰 둔다.
    let app = app.clone();
    std::thread::spawn(move || persist_cookies(&app));
}

fn content(app: &AppHandle, label: &str) -> Option<tauri::Webview> {
    app.get_webview(&content_label(label))
}

pub fn go_back(app: &AppHandle, label: &str) {
    if let Some(webview) = content(app, label) {
        let _ = webview.eval("history.back()");
    }
}

pub fn go_forward(app: &AppHandle, label: &str) {
    if let Some(webview) = content(app, label) {
        let _ = webview.eval("history.forward()");
    }
}

pub fn reload(app: &AppHandle, label: &str) {
    if let Some(webview) = content(app, label) {
        let _ = webview.reload();
    }
}

// MARK: - 로그인 쿠키

/// 처음 미리보기를 열 때 키체인의 쿠키를 저장소에 넣는다.
///
/// 키체인 읽기는 백그라운드에서 한다. 앱을 새로 설치하면 macOS 가 키체인 허용 창을 띄우는데,
/// 메인 스레드에서 기다리면 그동안 앱 전체가 멈추기 때문이다.
#[cfg(target_os = "macos")]
fn prepare_cookies(app: &AppHandle) {
    let state = sessions(app);
    let mut restored = lock(&state.restored);
    if *restored {
        return;
    }
    let now = linkgroupz_core::Timestamp::now();
    let cookies: Vec<_> =
        state.vault.load().unwrap_or_default().into_iter().filter(|cookie| !cookie.is_expired(now)).collect();
    crate::preview_cookies::set_cookies(app, cookies);
    *restored = true;
}

#[cfg(not(target_os = "macos"))]
fn prepare_cookies(_app: &AppHandle) {}

#[cfg(target_os = "macos")]
fn persist_cookies(app: &AppHandle) {
    prepare_cookies(app);
    let cookies = crate::preview_cookies::all_cookies(app);
    let _ = sessions(app).vault.save(&cookies, linkgroupz_core::Timestamp::now());
}

#[cfg(not(target_os = "macos"))]
fn persist_cookies(_app: &AppHandle) {}

/// 보관해 둔 로그인 쿠키의 사이트(도메인) 수. 셀 수 없는 운영체제면 None.
pub fn stored_site_count(app: &AppHandle) -> Option<usize> {
    #[cfg(target_os = "macos")]
    {
        let cookies = sessions(app).vault.load().unwrap_or_default();
        let sites: std::collections::HashSet<&str> = cookies.iter().map(|cookie| cookie.site()).collect();
        Some(sites.len())
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = app;
        None
    }
}

/// 미리보기 로그인 정보를 모두 지운다. 열려 있는 미리보기 창은 닫는다.
pub fn clear_logins(app: &AppHandle) {
    let labels: Vec<String> = lock(&sessions(app).infos).keys().cloned().collect();
    #[cfg(not(target_os = "macos"))]
    for label in &labels {
        if let Some(webview) = content(app, label) {
            let _ = webview.clear_all_browsing_data();
        }
    }
    for label in &labels {
        close(app, label);
    }
    #[cfg(target_os = "macos")]
    {
        let _ = sessions(app).vault.clear();
        crate::preview_cookies::clear(app);
    }
    #[cfg(not(target_os = "macos"))]
    {
        // 창이 닫히며 WebView2 가 폴더를 놓을 때까지 잠깐 기다린다.
        std::thread::sleep(Duration::from_millis(500));
        let _ = std::fs::remove_dir_all(profile_directory());
    }
}
