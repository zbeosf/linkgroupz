//! 앱의 창들: 관리 창 · 설정 · 시작 안내 · 바뀐 점 · 메뉴바(트레이) 창.

use crate::state::{lock, AppState};
use std::sync::atomic::Ordering;
use std::time::{Duration, Instant};
use tauri::{
    AppHandle, Emitter, LogicalSize, Manager, PhysicalPosition, Rect, WebviewUrl, WebviewWindow, WebviewWindowBuilder,
    WindowEvent,
};

pub const MANAGER: &str = "manager";
pub const SETTINGS: &str = "settings";
pub const WELCOME: &str = "welcome";
pub const WHATS_NEW: &str = "whatsnew";
pub const UPDATE_PROGRESS: &str = "update";
pub const MENUBAR: &str = "menubar";

/// 메뉴바 창의 폭 (높이는 내용에 맞춘다)
const POPOVER_WIDTH: f64 = 360.0;
const POPOVER_INITIAL_HEIGHT: f64 = 300.0;
/// 사용자가 줄일 수 있는 가장 작은 크기
const POPOVER_MIN_SIZE: (f64, f64) = (300.0, 200.0);
/// 메뉴바 창이 포커스를 잃어 닫힌 직후의 아이콘 클릭은 '닫기'로 본다 (다시 열지 않는다).
const POPOVER_REOPEN_GUARD: Duration = Duration::from_millis(350);
/// 메뉴바 창을 닫은 뒤 이만큼 다시 쓰지 않으면 창을 없앤다. 그 안에 다시 열면 바로 뜬다.
const POPOVER_KEEP_ALIVE: Duration = Duration::from_secs(30);

/// 창을 보이게 하고 맨 앞으로 가져온다. 메뉴바 앱은 창을 열어도 앞으로 나오지 않아 직접 활성화한다.
fn bring_to_front(window: &WebviewWindow) {
    let _ = window.unminimize();
    let _ = window.show();
    let _ = window.set_focus();
}

/// 이미 있으면 앞으로 가져오고, 없으면 만든다.
fn show_or_create(app: &AppHandle, label: &str, create: impl FnOnce() -> tauri::Result<WebviewWindow>) {
    match app.get_webview_window(label) {
        Some(window) => bring_to_front(&window),
        None => match create() {
            Ok(window) => bring_to_front(&window),
            Err(error) => eprintln!("{label} 창을 만들지 못했습니다: {error}"),
        },
    }
}

fn page(name: &str) -> WebviewUrl {
    WebviewUrl::App(name.into())
}

/// 관리 창. 처음 열 때(저장된 창 크기가 없을 때)의 크기는 1325×841 이고, 그 뒤에는 사용자가 바꾼 크기를 기억한다.
pub fn show_manager(app: &AppHandle) {
    show_or_create(app, MANAGER, || {
        let builder = WebviewWindowBuilder::new(app, MANAGER, page("index.html"))
            .title("LinkGroupz")
            .inner_size(1325.0, 841.0)
            .min_inner_size(720.0, 440.0)
            // 창 안에서 끌어다 놓기(순서 변경 · 다른 그룹으로 이동)를 쓴다.
            // 파일 끌어다 놓기 처리를 켜 두면 Windows 에서 화면의 끌어다 놓기가 동작하지 않는다.
            .disable_drag_drop_handler()
            .visible(false);
        // macOS: 기본 앱처럼 제목 줄을 화면 위에 겹치고(신호등 버튼은 왼쪽 목록 안), 창 뒤에 반투명 바탕을 깐다.
        // 화면은 반투명이 보일 곳(왼쪽 목록)만 비우고 나머지를 칠한다 (manager.css 의 macOS 부분).
        #[cfg(target_os = "macos")]
        let builder = builder
            .title_bar_style(tauri::TitleBarStyle::Overlay)
            .hidden_title(true)
            .transparent(true)
            .effects(tauri::utils::config::WindowEffectsConfig {
                effects: vec![tauri::window::Effect::Sidebar],
                state: Some(tauri::window::EffectState::FollowsWindowActiveState),
                ..Default::default()
            });
        let window = builder.build()?;
        #[cfg(target_os = "macos")]
        use_unified_toolbar(&window);
        Ok(window)
    });
}

/// 빈 툴바를 붙여 제목 줄을 macOS 기본 앱(SwiftUI)처럼 높게 만든다. 신호등 버튼이 그 높이의 가운데로 내려와
/// 둥글게 띄운 왼쪽 목록 안에 들어간다. (Tauri 의 신호등 위치 지정은 macOS 26 에서 먹지 않는다.)
#[cfg(target_os = "macos")]
fn use_unified_toolbar(window: &WebviewWindow) {
    let Ok(ns_window) = window.ns_window() else {
        return;
    };
    let ns_window = ns_window as usize;
    let _ = window.run_on_main_thread(move || {
        use objc2::rc::Retained;
        use objc2::runtime::AnyObject;
        use objc2::{class, msg_send};
        /// NSWindowToolbarStyleUnified
        const UNIFIED: isize = 3;
        // Safety: 메인 스레드에서 살아 있는 창(NSWindow)에 공개 메서드만 부른다. 툴바는 창이 붙잡는다.
        unsafe {
            let ns_window = ns_window as *mut AnyObject;
            let toolbar: Retained<AnyObject> = msg_send![class!(NSToolbar), new];
            let _: () = msg_send![ns_window, setToolbar: &*toolbar];
            let _: () = msg_send![ns_window, setToolbarStyle: UNIFIED];
        }
    });
}

/// 관리 창이 열려 있으면 맨 앞으로 가져온다 (브라우저에 앞 순서를 빼앗긴 뒤).
pub fn focus_manager(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MANAGER) {
        let _ = window.set_focus();
    }
}

pub fn show_settings(app: &AppHandle) {
    show_or_create(app, SETTINGS, || {
        WebviewWindowBuilder::new(app, SETTINGS, page("settings.html"))
            .title(settings_title())
            .inner_size(580.0, 720.0)
            .min_inner_size(520.0, 420.0)
            .maximizable(false)
            .visible(false)
            .build()
    });
}

pub fn show_welcome(app: &AppHandle) {
    show_or_create(app, WELCOME, || {
        WebviewWindowBuilder::new(app, WELCOME, page("welcome.html"))
            .title(welcome_title())
            .inner_size(660.0, 640.0)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .center()
            .visible(false)
            .build()
    });
}

/// '바뀐 점' 창. 보여 줄 내용(`AppState::whats_new`)이 달라질 수 있어 열려 있던 창은 닫고 새로 연다.
pub fn show_whats_new(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(WHATS_NEW) {
        let _ = window.destroy();
    }
    let created = WebviewWindowBuilder::new(app, WHATS_NEW, page("whatsnew.html"))
        .title(whats_new_title())
        .inner_size(520.0, 480.0)
        .resizable(false)
        .maximizable(false)
        .minimizable(false)
        .center()
        .visible(false)
        .build();
    match created {
        Ok(window) => bring_to_front(&window),
        Err(error) => eprintln!("바뀐 점 창을 만들지 못했습니다: {error}"),
    }
}

fn settings_title() -> &'static str {
    linkgroupz_core::tr!("LinkGroupz 설정", "LinkGroupz Settings")
}

fn welcome_title() -> &'static str {
    linkgroupz_core::tr!("LinkGroupz 시작하기", "Welcome to LinkGroupz")
}

fn whats_new_title() -> &'static str {
    linkgroupz_core::tr!("바뀐 점", "What's New")
}

fn update_title() -> &'static str {
    linkgroupz_core::tr!("LinkGroupz 업데이트", "LinkGroupz Update")
}

/// 언어를 바꾼 뒤 열려 있는 창의 제목을 바꾼다.
pub fn retitle_windows(app: &AppHandle) {
    for (label, title) in [(SETTINGS, settings_title()), (WELCOME, welcome_title()), (WHATS_NEW, whats_new_title()), (UPDATE_PROGRESS, update_title())] {
        if let Some(window) = app.get_webview_window(label) {
            let _ = window.set_title(title);
        }
    }
}

/// 업데이트 진행 창: 새 버전을 내려받고 설치하는 동안 진행 막대를 보여 준다 (macOS · Windows 같다).
/// 설치가 끝나면 앱이 다시 켜지며 함께 닫힌다. 실패하면 이유와 '닫기'를 보여 준다 (update.js).
pub fn show_update_progress(app: &AppHandle) {
    show_or_create(app, UPDATE_PROGRESS, || {
        WebviewWindowBuilder::new(app, UPDATE_PROGRESS, page("update.html"))
            .title(update_title())
            .inner_size(380.0, 150.0)
            .resizable(false)
            .maximizable(false)
            .minimizable(false)
            .center()
            .visible(false)
            .build()
    });
}

/// 화면에 보이는 높이가 내용 높이와 같아지도록 창 높이를 바꾼다.
///
/// content 는 내용의 높이, viewport 는 지금 화면(웹뷰)에 보이는 높이다. 창 높이를 내용 높이로 그대로 잡으면
/// 제목 표시줄이 차지하는 만큼 아래가 잘린다 — 제목 표시줄 높이는 운영체제 · 버전마다 다르고 창 크기에
/// 들어가는지도 달라, 지금 창 높이에서 모자란(남는) 만큼만 더하거나 뺀다.
pub fn fit_height(window: &WebviewWindow, content: f64, viewport: f64) {
    let Ok(scale) = window.scale_factor() else {
        return;
    };
    let Ok(size) = window.inner_size() else {
        return;
    };
    let size = size.to_logical::<f64>(scale);
    // 내용이 아주 길면 화면 밖으로 나가지 않게 여기까지만 키우고, 나머지는 창 안에서 스크롤한다.
    let visible = content.clamp(120.0, 900.0);
    let height = size.height + (visible - viewport);
    if (height - size.height).abs() < 0.5 {
        return;
    }
    let _ = window.set_size(LogicalSize::new(size.width, height));
    if window.label() != MENUBAR {
        let _ = window.center();
    }
}

// MARK: - 메뉴바(트레이) 창

/// 메뉴바 창의 둥근 모서리 (menubar.css 의 #app 과 같게)
#[cfg(target_os = "macos")]
const POPOVER_RADIUS: f64 = 12.0;

/// 메뉴바 창을 만든다 (숨긴 채로). 화면이 내용 높이를 알려 오면(`resize_popover`) 그때 띄운다.
fn build_popover(app: &AppHandle) -> tauri::Result<WebviewWindow> {
    let builder = WebviewWindowBuilder::new(app, MENUBAR, page("menubar.html"))
        .title("LinkGroupz")
        .inner_size(popover_width(app), POPOVER_INITIAL_HEIGHT)
        .min_inner_size(POPOVER_MIN_SIZE.0, POPOVER_MIN_SIZE.1)
        // 가장자리를 끌어 크기를 바꿀 수 있다 (바꾼 크기로 계속 연다).
        .resizable(true)
        .decorations(false)
        // 메뉴바에 붙어 있을 때만 다른 창 위에 뜬다. 떼어 놓으면 보통 창처럼 다른 창 뒤로 갈 수 있다.
        .always_on_top(!popover_detached(app))
        .skip_taskbar(true)
        .visible_on_all_workspaces(true)
        .shadow(true)
        // 만들 때는 앞으로 가져오지 않는다. 숨긴 창을 만들며 앱을 앞으로 가져오면 관리 · 설정 · 안내 창이 함께 튀어나온다.
        // 띄울 때(present_popover) 이 창만 앞으로 가져온다.
        .focused(false)
        .visible(false);
    // 둥근 모서리를 화면(CSS)에서 그리도록 창 배경을 투명하게 하고, 그 뒤에 macOS 메뉴바 창(원작의 SwiftUI MenuBarExtra)과 같은
    // 반투명 바탕을 같은 둥근 모서리로 깐다. 화면은 바탕을 반쯤 비치게 칠한다 (menubar.css).
    #[cfg(target_os = "macos")]
    let builder = builder.transparent(true).effects(tauri::utils::config::WindowEffectsConfig {
        effects: vec![tauri::window::Effect::Popover],
        state: Some(tauri::window::EffectState::Active),
        radius: Some(POPOVER_RADIUS),
        ..Default::default()
    });
    let window = builder.build()?;

    let app = app.clone();
    let watched = window.clone();
    window.on_window_event(move |event| {
        match event {
            // 메뉴바에 붙어 있으면 다른 곳을 누를 때 닫힌다. 떼어 놓았으면 그대로 둔다.
            WindowEvent::Focused(false) => {
                if watched.is_visible().unwrap_or(false) && !popover_detached(&app) {
                    dismiss_popover(&app, &watched, true);
                }
            }
            WindowEvent::Resized(size) if watched.is_visible().unwrap_or(false) => popover_resized_by_user(&app, &watched, *size),
            WindowEvent::Moved(position) if watched.is_visible().unwrap_or(false) && popover_detached(&app) => {
                remember_popover_position(&app, *position)
            }
            _ => {}
        }
    });
    Ok(window)
}

/// 메뉴바 아이콘 클릭: 창이 떠 있으면 닫고, 아니면 아이콘 아래(작업 표시줄이 아래면 위)에 띄운다.
pub fn toggle_popover(app: &AppHandle, icon: Rect) {
    let state = app.state::<AppState>();
    let existing = app.get_webview_window(MENUBAR);
    if let Some(window) = &existing {
        if window.is_visible().unwrap_or(false) {
            // 떼어 놓은 창이 다른 창 뒤에 있으면 앞으로 가져오기만 한다.
            if popover_detached(app) && !window.is_focused().unwrap_or(false) {
                let _ = window.set_focus();
                return;
            }
            dismiss_popover(app, window, false);
            return;
        }
    }
    let just_hidden = lock(&state.popover.hidden_at).is_some_and(|at| at.elapsed() < POPOVER_REOPEN_GUARD);
    if just_hidden {
        return;
    }
    match existing {
        // 닫은 지 얼마 안 돼 아직 남아 있는 창은 바로 다시 띄운다.
        Some(window) => present_popover(app, &window, icon),
        None => {
            if lock(&state.popover.pending_icon).replace(icon).is_some() {
                // 이미 만드는 중이다.
                return;
            }
            // 웹뷰 창을 이벤트 처리 중(메인 스레드)에 만들면 Windows 에서 멈춘다 (WebView2). 다른 스레드에서 만든다.
            let app = app.clone();
            std::thread::spawn(move || {
                if let Err(error) = build_popover(&app) {
                    eprintln!("메뉴바 창을 만들지 못했습니다: {error}");
                    lock(&app.state::<AppState>().popover.pending_icon).take();
                }
            });
        }
    }
}

/// 창을 아이콘 자리에(떼어 놓았으면 마지막 자리에) 놓고 띄운다. 예약해 둔 '없애기'는 무효가 된다.
fn present_popover(app: &AppHandle, window: &WebviewWindow, icon: Rect) {
    app.state::<AppState>().popover.generation.fetch_add(1, Ordering::SeqCst);
    // 설정을 한 번만 잠근다 (같은 식 안에서 다시 잠그면 앞의 잠금이 풀리지 않아 멈춘다).
    let saved = {
        let state = app.state::<AppState>();
        let settings = lock(&state.settings);
        settings.popover_position.filter(|_| settings.popover_detached)
    };
    // 모니터를 빼서 그 자리가 화면 밖이면 아이콘 아래에 띄운다.
    let on_screen = saved.filter(|(x, y)| app.monitor_from_point(f64::from(*x) + 20.0, f64::from(*y) + 20.0).ok().flatten().is_some());
    match on_screen {
        Some((x, y)) => {
            let _ = window.set_position(PhysicalPosition::new(x, y));
        }
        None => place_popover(app, window, icon),
    }
    let _ = window.show();
    let _ = window.set_focus();
    let _ = window.emit("popover-shown", popover_layout(app));
}

/// 창을 닫는다(숨긴다). 한동안 다시 쓰지 않으면 창을 없애 웹뷰 프로세스를 돌려준다.
fn dismiss_popover(app: &AppHandle, window: &WebviewWindow, by_focus_loss: bool) {
    let _ = window.hide();
    let state = app.state::<AppState>();
    if by_focus_loss {
        *lock(&state.popover.hidden_at) = Some(Instant::now());
    }
    let generation = state.popover.generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(POPOVER_KEEP_ALIVE).await;
        if app.state::<AppState>().popover.generation.load(Ordering::SeqCst) != generation {
            return;
        }
        if let Some(window) = app.get_webview_window(MENUBAR) {
            if !window.is_visible().unwrap_or(false) {
                let _ = window.destroy();
            }
        }
    });
}

/// 창을 아이콘에 맞춰 놓는다. 화면 밖으로 나가지 않게 한다.
fn place_popover(app: &AppHandle, window: &WebviewWindow, icon: Rect) {
    let scale_hint = window.scale_factor().unwrap_or(1.0);
    let icon_position = icon.position.to_physical::<f64>(scale_hint);
    let icon_size = icon.size.to_physical::<f64>(scale_hint);
    let icon_center_x = icon_position.x + icon_size.width / 2.0;
    let icon_center_y = icon_position.y + icon_size.height / 2.0;

    let monitor = app.monitor_from_point(icon_center_x, icon_center_y).ok().flatten();
    let Ok(size) = window.outer_size() else {
        return;
    };
    let (width, height) = (f64::from(size.width), f64::from(size.height));
    let gap = 6.0 * monitor.as_ref().map_or(scale_hint, |monitor| monitor.scale_factor());

    let mut x = icon_center_x - width / 2.0;
    let mut y = icon_position.y + icon_size.height + gap;
    if let Some(monitor) = monitor {
        let area = monitor.work_area();
        let (left, top) = (f64::from(area.position.x), f64::from(area.position.y));
        let (right, bottom) = (left + f64::from(area.size.width), top + f64::from(area.size.height));
        // 아이콘이 화면 아래쪽(Windows 작업 표시줄)에 있으면 창을 아이콘 위에 띄운다.
        if icon_center_y > (top + bottom) / 2.0 {
            y = icon_position.y - height - gap;
        }
        x = x.clamp(left + gap, (right - width - gap).max(left + gap));
        y = y.clamp(top, (bottom - height).max(top));
    }
    let _ = window.set_position(PhysicalPosition::new(x.round() as i32, y.round() as i32));
}

fn popover_detached(app: &AppHandle) -> bool {
    lock(&app.state::<AppState>().settings).popover_detached
}

/// 메뉴바에서 떼어 놓거나(detached) 다시 붙인다. 붙이면 아이콘 아래로 돌아간다.
pub fn set_popover_detached(app: &AppHandle, detached: bool) {
    let window = app.get_webview_window(MENUBAR);
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        settings.popover_detached = detached;
        if detached {
            if let Some(position) = window.as_ref().and_then(|window| window.outer_position().ok()) {
                settings.popover_position = Some((position.x, position.y));
            }
        }
        settings.save();
    }
    let Some(window) = window else {
        return;
    };
    let _ = window.set_always_on_top(!detached);
    if !detached {
        if let Some(icon) = tray_icon_rect(app) {
            place_popover(app, &window, icon);
        }
        let _ = window.set_focus();
    }
    let _ = window.emit("popover-layout", popover_layout(app));
}

/// 떼어 놓은 창을 옮기면 그 자리를 기억한다 (옮기는 동안 여러 번 오므로 파일은 멈춘 뒤 한 번 쓴다).
fn remember_popover_position(app: &AppHandle, position: PhysicalPosition<i32>) {
    lock(&app.state::<AppState>().settings).popover_position = Some((position.x, position.y));
    let generation = app.state::<AppState>().popover.resize_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        if app.state::<AppState>().popover.resize_generation.load(Ordering::SeqCst) == generation {
            lock(&app.state::<AppState>().settings).save();
        }
    });
}

/// 메뉴바(트레이) 아이콘의 자리. 메뉴바가 꽉 차 아이콘이 화면 밖으로 밀려나 있으면 None.
fn tray_icon_rect(app: &AppHandle) -> Option<Rect> {
    let icon = app.tray_by_id(crate::tray::TRAY_ID).and_then(|tray| tray.rect().ok().flatten())?;
    let scale = app.primary_monitor().ok().flatten().map_or(1.0, |monitor| monitor.scale_factor());
    let position = icon.position.to_physical::<f64>(scale);
    let size = icon.size.to_physical::<f64>(scale);
    app.monitor_from_point(position.x + size.width / 2.0, position.y + size.height / 2.0).ok().flatten().map(|_| icon)
}

/// 메뉴바 창이 아이콘 위에 떠 있는지 (Windows 작업 표시줄이 아래에 있을 때). 그때는 높이가 바뀌어도 아래쪽 끝을 지킨다.
/// 떼어 놓은 창은 아이콘과 상관없이 위쪽 끝을 지킨다.
fn popover_anchored_above(app: &AppHandle, window: &WebviewWindow) -> bool {
    if popover_detached(app) {
        return false;
    }
    let (Ok(position), Ok(size)) = (window.outer_position(), window.outer_size()) else {
        return false;
    };
    app.monitor_from_point(f64::from(position.x), f64::from(position.y)).ok().flatten().is_some_and(|monitor| {
        let area = monitor.work_area();
        let middle = f64::from(area.position.y) + f64::from(area.size.height) / 2.0;
        f64::from(position.y) + f64::from(size.height) / 2.0 > middle
    })
}

/// 사용자가 정한 메뉴바 창 크기 (없으면 폭은 기본, 높이는 내용에 맞춘다)
fn chosen_popover_size(app: &AppHandle) -> Option<(f64, f64)> {
    lock(&app.state::<AppState>().settings).popover_size
}

fn popover_width(app: &AppHandle) -> f64 {
    chosen_popover_size(app).map_or(POPOVER_WIDTH, |(width, _)| width)
}

/// 너무 작아 쓸 수 없거나 화면 밖으로 나가지 않게 한다.
fn clamp_popover_size(app: &AppHandle, window: &WebviewWindow, (width, height): (f64, f64)) -> (f64, f64) {
    let area = window
        .outer_position()
        .ok()
        .and_then(|position| app.monitor_from_point(f64::from(position.x), f64::from(position.y)).ok().flatten())
        .or_else(|| app.primary_monitor().ok().flatten())
        .map(|monitor| {
            let size = monitor.work_area().size.to_logical::<f64>(monitor.scale_factor());
            (size.width, size.height)
        })
        .unwrap_or((1600.0, 900.0));
    (
        width.clamp(POPOVER_MIN_SIZE.0, (area.0 - 24.0).max(POPOVER_MIN_SIZE.0)),
        height.clamp(POPOVER_MIN_SIZE.1, (area.1 - 24.0).max(POPOVER_MIN_SIZE.1)),
    )
}

/// 창 크기를 바꾼다 (앱이 바꾸는 것이라 사용자가 끈 것으로 보지 않게 기억해 둔다).
/// 떠 있는 창이 아이콘 위에 있으면 아래쪽 끝을 지킨다.
fn apply_popover_size(app: &AppHandle, window: &WebviewWindow, (width, height): (f64, f64)) {
    let (Ok(scale), Ok(position), Ok(old_size)) = (window.scale_factor(), window.outer_position(), window.outer_size()) else {
        return;
    };
    let anchored_above = window.is_visible().unwrap_or(false) && popover_anchored_above(app, window);
    let target = LogicalSize::new(width, height).to_physical::<u32>(scale);
    if target == old_size {
        return;
    }
    *lock(&app.state::<AppState>().popover.expected_size) = Some(target);
    let _ = window.set_size(LogicalSize::new(width, height));
    if anchored_above {
        let bottom = position.y + old_size.height as i32;
        let _ = window.set_position(PhysicalPosition::new(position.x, bottom - target.height as i32));
    }
}

/// 메뉴바 창의 내용 높이가 바뀌면 창 높이를 맞춘다. 사용자가 크기를 정해 뒀으면 그 크기를 쓴다.
/// 방금 만든 창이면 크기를 맞춘 뒤 띄운다.
pub fn resize_popover(app: &AppHandle, height: f64) {
    let Some(window) = app.get_webview_window(MENUBAR) else {
        return;
    };
    let size = match chosen_popover_size(app) {
        Some(size) => clamp_popover_size(app, &window, size),
        None => (popover_width(app), height.clamp(120.0, 640.0)),
    };
    apply_popover_size(app, &window, size);
    let pending = lock(&app.state::<AppState>().popover.pending_icon).take();
    if let Some(icon) = pending {
        present_popover(app, &window, icon);
    }
}

/// 사용자가 손잡이를 끌어 크기를 정한다 (None 이면 다시 내용에 맞춘다). done 이면 설정 파일에 저장한다.
pub fn set_popover_size(app: &AppHandle, size: Option<(f64, f64)>, done: bool) {
    let window = app.get_webview_window(MENUBAR);
    let size = match (&window, size) {
        (Some(window), Some(size)) => Some(clamp_popover_size(app, window, size)),
        (_, size) => size,
    };
    remember_popover_size(app, size, done);
    if let (Some(window), Some(size)) = (&window, size) {
        apply_popover_size(app, window, size);
    }
}

fn remember_popover_size(app: &AppHandle, size: Option<(f64, f64)>, save: bool) {
    let state = app.state::<AppState>();
    let mut settings = lock(&state.settings);
    settings.popover_size = size.map(|(width, height)| (width.round(), height.round()));
    if save {
        settings.save();
    }
}

/// 사용자가 창 가장자리를 끌어 크기를 바꿨다: 그 크기를 기억하고 화면에 '크기 고정'을 알린다.
/// 끄는 동안 여러 번 오므로 설정 파일은 잠시 멈춘 뒤 한 번 쓴다.
fn popover_resized_by_user(app: &AppHandle, window: &WebviewWindow, size: tauri::PhysicalSize<u32>) {
    {
        let state = app.state::<AppState>();
        let mut expected = lock(&state.popover.expected_size);
        if expected.is_some_and(|expected| expected == size) {
            *expected = None;
            return;
        }
    }
    let Ok(scale) = window.scale_factor() else {
        return;
    };
    let logical = size.to_logical::<f64>(scale);
    remember_popover_size(app, Some((logical.width, logical.height)), false);
    let _ = window.emit("popover-layout", popover_layout(app));
    let generation = app.state::<AppState>().popover.resize_generation.fetch_add(1, Ordering::SeqCst) + 1;
    let app = app.clone();
    tauri::async_runtime::spawn(async move {
        tokio::time::sleep(Duration::from_millis(600)).await;
        if app.state::<AppState>().popover.resize_generation.load(Ordering::SeqCst) == generation {
            lock(&app.state::<AppState>().settings).save();
        }
    });
}

/// 화면이 그릴 때 묻는다: 사용자가 크기를 정했는지와, 창이 아이콘 위에 떠 있는지.
pub fn popover_layout(app: &AppHandle) -> PopoverLayout {
    PopoverLayout {
        fixed: chosen_popover_size(app).is_some(),
        detached: popover_detached(app),
        anchored_above: app.get_webview_window(MENUBAR).is_some_and(|window| popover_anchored_above(app, &window)),
    }
}

#[derive(Clone, serde::Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PopoverLayout {
    /// 사용자가 크기를 정해 두었다 (내용에 맞추지 않고, 목록이 창 안에서 스크롤된다)
    pub fixed: bool,
    /// 메뉴바에서 떼어 놓았다 (끌어서 옮기고, 다른 곳을 눌러도 닫히지 않는다)
    pub detached: bool,
    /// 창이 아이콘 위에 떠 있다 (크기 손잡이를 위쪽에 둔다)
    pub anchored_above: bool,
}

pub fn hide_popover(app: &AppHandle) {
    if let Some(window) = app.get_webview_window(MENUBAR) {
        if window.is_visible().unwrap_or(false) {
            dismiss_popover(app, &window, false);
        }
    }
}

/// 단축키로: 아이콘을 누른 것처럼 메뉴바 창을 여닫는다. 창은 아이콘 자리에 뜬다.
/// 단축키를 눌렀다: 메뉴바(트레이) 창을 연다. 메뉴바 창이 이미 열려 있으면 닫고 관리 창을 연다.
pub fn shortcut_pressed(app: &AppHandle) {
    let open = app.get_webview_window(MENUBAR).is_some_and(|window| window.is_visible().unwrap_or(false));
    if open {
        hide_popover(app);
        show_manager(app);
    } else {
        toggle_popover_at_tray(app);
    }
}

/// 앱을 켤 때 메뉴바(트레이) 창을 띄운다 (이미 떠 있으면 그대로 둔다).
pub fn show_popover_at_tray(app: &AppHandle) {
    if app.get_webview_window(MENUBAR).is_some_and(|window| window.is_visible().unwrap_or(false)) {
        return;
    }
    toggle_popover_at_tray(app);
}

pub fn toggle_popover_at_tray(app: &AppHandle) {
    // 메뉴바가 꽉 차면 macOS 는 아이콘을 화면 밖으로 밀어 둔다. 그 자리에 창을 띄우면 보이지 않으니,
    // 아이콘이 화면 안에 없으면 마우스가 있는 화면의 아이콘이 있을 만한 자리에 띄운다.
    let icon = tray_icon_rect(app);
    let monitor_under_cursor = || {
        let cursor = app.cursor_position().ok()?;
        app.monitor_from_point(cursor.x, cursor.y).ok().flatten()
    };
    let icon = icon.or_else(|| monitor_under_cursor().or_else(|| app.primary_monitor().ok().flatten()).map(|monitor| assumed_icon(&monitor)));
    if let Some(icon) = icon {
        toggle_popover(app, icon);
    }
}

/// 아이콘이 있을 만한 자리: macOS 는 화면 오른쪽 위(메뉴바), 그 밖에는 오른쪽 아래(작업 표시줄).
fn assumed_icon(monitor: &tauri::Monitor) -> Rect {
    let area = monitor.work_area();
    let side = 24.0 * monitor.scale_factor();
    let x = f64::from(area.position.x) + f64::from(area.size.width) - 12.0 * side;
    let y = if cfg!(target_os = "macos") {
        f64::from(area.position.y) - side
    } else {
        f64::from(area.position.y) + f64::from(area.size.height)
    };
    Rect {
        position: PhysicalPosition::new(x, y).into(),
        size: tauri::PhysicalSize::new(side, side).into(),
    }
}

/// 사용법 스크린샷 · 확인용: 아이콘을 누르지 않고 메뉴바 창을 띄운다.
pub fn show_popover_for_screenshot(app: &AppHandle) {
    if let Ok(Some(monitor)) = app.primary_monitor() {
        toggle_popover(app, assumed_icon(&monitor));
    }
}
