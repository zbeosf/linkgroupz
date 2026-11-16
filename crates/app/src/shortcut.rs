//! 메뉴바(트레이) 창을 여닫는 전역 단축키. 다른 프로그램을 쓰는 중에도 듣는다.
//!
//! 기본은 없음이고 설정 창에서 정한다. 단축키는 `Control+Alt+Shift+Super+KeyK` 처럼
//! 보조 키 이름과 키 이름(화면의 `KeyboardEvent.code`)을 `+` 로 이은 글자로 주고받고 저장한다.

use crate::settings::Settings;
use crate::state::{lock, AppState};
use crate::windows;
use serde::Serialize;
use tauri::plugin::TauriPlugin;
use tauri::{AppHandle, Manager, Wry};
use tauri_plugin_global_shortcut::{Code, GlobalShortcutExt, Modifiers, Shortcut, ShortcutState};

#[derive(Debug, PartialEq, Eq, thiserror::Error)]
pub enum ShortcutError {
    #[error("{}", linkgroupz_core::tr!("쓸 수 없는 키입니다. 다른 키로 해 주세요.", "This key can't be used. Please try another."))]
    Unreadable,
    #[error("{}", linkgroupz_core::tr!("Ctrl · Alt · ⌘(Windows 키) 중 하나와 함께 눌러 주세요.", "Press it together with Ctrl, Alt or ⌘ (the Windows key)."))]
    NeedsModifier,
    #[error("{}", linkgroupz_core::tr!("다른 프로그램이나 시스템이 이미 쓰는 단축키라 등록하지 못했습니다.", "Couldn't register it because another app or the system already uses this shortcut."))]
    Taken,
}

/// 화면에 보내는 단축키 상태
#[derive(Clone, Debug, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ShortcutStatus {
    /// 정해 둔 단축키 (없으면 None)
    pub shortcut: Option<String>,
    /// 지금 실제로 듣고 있는지. 정해 뒀는데 false 면 다른 프로그램이 먼저 차지한 것이다.
    pub active: bool,
}

/// 단축키를 누르면 메뉴바 창을 여닫는다.
pub fn plugin() -> TauriPlugin<Wry> {
    tauri_plugin_global_shortcut::Builder::new()
        .with_handler(|app, _shortcut, event| {
            if event.state() == ShortcutState::Pressed {
                windows::shortcut_pressed(app);
            }
        })
        .build()
}

/// 글자를 단축키로 읽는다. 글자 키 하나만으로는(보조 키 없이) 받지 않는다 — 글을 쓸 때마다 걸린다.
pub fn parse(text: &str) -> Result<Shortcut, ShortcutError> {
    let shortcut: Shortcut = text.trim().parse().map_err(|_| ShortcutError::Unreadable)?;
    let has_modifier = shortcut.mods.intersects(Modifiers::CONTROL | Modifiers::ALT | Modifiers::SUPER);
    if !has_modifier && !is_function_key(shortcut.key) {
        return Err(ShortcutError::NeedsModifier);
    }
    Ok(shortcut)
}

fn is_function_key(key: Code) -> bool {
    // F1 … F24 는 글을 쓸 때 누르지 않으므로 보조 키 없이도 받는다.
    let name = key.to_string();
    name.strip_prefix('F').is_some_and(|number| !number.is_empty() && number.chars().all(|c| c.is_ascii_digit()))
}

/// 듣던 단축키를 모두 풀고, shortcut 이 있으면 그것을 듣는다.
fn listen(app: &AppHandle, shortcut: Option<&str>) -> Result<(), ShortcutError> {
    let manager = app.global_shortcut();
    let _ = manager.unregister_all();
    if let Some(text) = shortcut {
        manager.register(parse(text)?).map_err(|_| ShortcutError::Taken)?;
    }
    Ok(())
}

fn saved(app: &AppHandle) -> Option<String> {
    lock(&app.state::<AppState>().settings).popover_shortcut.clone()
}

pub fn status(app: &AppHandle) -> ShortcutStatus {
    let shortcut = saved(app);
    let active = shortcut.as_deref().and_then(|text| parse(text).ok()).is_some_and(|parsed| app.global_shortcut().is_registered(parsed));
    ShortcutStatus { shortcut, active }
}

/// 앱이 켜질 때: 정해 둔 단축키를 듣기 시작한다. 다른 프로그램이 차지했으면 설정 창에 표시된다.
pub fn start(app: &AppHandle) {
    if let Err(error) = listen(app, saved(app).as_deref()) {
        eprintln!("단축키를 등록하지 못했습니다: {error}");
    }
}

/// 단축키를 바꾼다 (None 이면 없앤다). 등록하지 못하면 예전 것을 그대로 둔다.
pub fn set(app: &AppHandle, shortcut: Option<String>) -> Result<ShortcutStatus, ShortcutError> {
    let shortcut = shortcut.map(|text| text.trim().to_owned()).filter(|text| !text.is_empty());
    if let Err(error) = listen(app, shortcut.as_deref()) {
        let _ = listen(app, saved(app).as_deref());
        return Err(error);
    }
    {
        let state = app.state::<AppState>();
        let mut settings: std::sync::MutexGuard<'_, Settings> = lock(&state.settings);
        settings.popover_shortcut = shortcut;
        settings.save();
    }
    Ok(status(app))
}

/// 설정 창에서 새 단축키를 받는 동안은 듣지 않는다 (지금 단축키를 다시 눌러도 창이 뜨지 않고 그 키가 입력되게).
pub fn pause(app: &AppHandle, paused: bool) {
    let _ = listen(app, if paused { None } else { saved(app) }.as_deref());
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn shortcut_text() {
        for ok in ["Alt+Space", "Control+Shift+KeyK", "Super+Alt+Digit1", "Control+Alt+Shift+Super+ArrowUp", "F5", "Shift+F12", "Alt+Backquote"] {
            assert!(parse(ok).is_ok(), "{ok}");
        }
        assert_eq!(parse("KeyK"), Err(ShortcutError::NeedsModifier), "글자 키만으로는 안 됨");
        assert_eq!(parse("Shift+KeyK"), Err(ShortcutError::NeedsModifier), "Shift 만으로는 안 됨 (대문자 입력)");
        assert_eq!(parse("Control+"), Err(ShortcutError::Unreadable));
        assert_eq!(parse("Control+Nope"), Err(ShortcutError::Unreadable));
        assert_eq!(parse(""), Err(ShortcutError::Unreadable));
        assert_eq!(parse(" Alt+Space ").map(|shortcut| shortcut.key), Ok(Code::Space), "앞뒤 공백은 뗀다");
    }
}
