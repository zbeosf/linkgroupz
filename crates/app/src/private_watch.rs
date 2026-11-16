//! 관리 창에서 연 시크릿 창을 닫으면 관리 창을 다시 맨 앞으로 가져온다 (macOS).
//!
//! 브라우저 창을 닫으면 macOS 는 그 브라우저의 다른 창을 앞으로 가져온다. 관리 창에서 시크릿 창을 열어 보고 닫은 뒤
//! 다시 관리 창으로 돌아오기 쉽게, 그 시크릿 창이 닫히는 것을 지켜본다. 창 번호만 보므로 화면 기록 권한은 필요 없다.
//! 다른 운영체제는 창 번호를 읽지 않아 아무 일도 하지 않는다.

use crate::windows;
use linkgroupz_core::{Browser, LauncherError};
use std::collections::HashSet;
use std::time::{Duration, Instant};
use tauri::{AppHandle, Manager};

/// 새 창이 뜨기를 기다리는 시간
const APPEAR_TIMEOUT: Duration = Duration::from_secs(8);
/// 닫히기를 지켜보는 최대 시간
const WATCH_LIMIT: Duration = Duration::from_secs(12 * 60 * 60);

/// open 으로 브라우저 창을 연다. 관리 창에서 연 것(from_manager)이면 그 창이 닫힐 때 관리 창으로 돌아온다.
pub fn open_watching(
    app: &AppHandle,
    browser: Browser,
    from_manager: bool,
    open: impl FnOnce() -> Result<(), LauncherError>,
) -> Result<(), LauncherError> {
    if !from_manager || !cfg!(target_os = "macos") {
        return open();
    }
    let before = browser.window_numbers();
    open()?;
    let app = app.clone();
    std::thread::spawn(move || watch(&app, browser, &before));
    Ok(())
}

fn watch(app: &AppHandle, browser: Browser, before: &HashSet<u32>) {
    let started = Instant::now();
    let opened: HashSet<u32> = loop {
        let new: HashSet<u32> = browser.window_numbers().difference(before).copied().collect();
        if !new.is_empty() {
            break new;
        }
        if started.elapsed() > APPEAR_TIMEOUT {
            return;
        }
        std::thread::sleep(Duration::from_millis(200));
    };
    while started.elapsed() < WATCH_LIMIT {
        std::thread::sleep(Duration::from_millis(700));
        if browser.window_numbers().is_disjoint(&opened) {
            // 사용자가 브라우저에서 그 창을 닫은 경우에만 돌아온다 (다른 앱을 쓰는 중이면 끼어들지 않는다).
            if !browser.is_frontmost() && browser.is_running() {
                return;
            }
            let Some(manager) = app.get_webview_window(windows::MANAGER) else {
                return;
            };
            if manager.is_visible().unwrap_or(false) && !manager.is_minimized().unwrap_or(false) {
                windows::show_manager(app);
            }
            return;
        }
    }
}
