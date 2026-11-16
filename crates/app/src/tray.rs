use linkgroupz_core::tr;
use crate::windows;
use tauri::image::Image;
use tauri::menu::{CheckMenuItem, Menu, MenuItem, PredefinedMenuItem, Submenu};
use tauri::tray::{MouseButton, MouseButtonState, TrayIconBuilder, TrayIconEvent};
use tauri::AppHandle;

pub const TRAY_ID: &str = "main";

/// 메뉴바(Windows 는 트레이) 아이콘.
///
/// 왼쪽 클릭: 그룹을 검색하고 바로 여는 작은 창. 오른쪽 클릭: 관리 창 · 프로필 · 설정 · 종료 메뉴.
pub fn create(app: &AppHandle) -> tauri::Result<()> {
    let menu = build_menu(app)?;

    // macOS 는 템플릿(검정 + 투명) 그림을 써서 밝은 · 어두운 메뉴바에 맞춰 색이 바뀐다.
    let icon = if cfg!(target_os = "macos") {
        Image::from_bytes(include_bytes!("../icons/tray-template.png"))?
    } else {
        Image::from_bytes(include_bytes!("../icons/tray-color.png"))?
    };

    TrayIconBuilder::with_id(TRAY_ID)
        .icon(icon)
        .icon_as_template(cfg!(target_os = "macos"))
        .tooltip("LinkGroupz")
        .menu(&menu)
        .show_menu_on_left_click(false)
        .on_menu_event(|app, event| match event.id().as_ref() {
            "manager" => windows::show_manager(app),
            "settings" | "profiles" => windows::show_settings(app),
            "welcome" => windows::show_welcome(app),
            "quit" => app.exit(0),
            id => {
                if let Some(profile) = id.strip_prefix(PROFILE_PREFIX) {
                    let _ = crate::profiles::switch_to(app, profile);
                }
            }
        })
        .on_tray_icon_event(|tray, event| {
            if let TrayIconEvent::Click { button: MouseButton::Left, button_state: MouseButtonState::Up, rect, .. } = event {
                windows::toggle_popover(tray.app_handle(), rect);
            }
        })
        .build(app)?;
    Ok(())
}

const PROFILE_PREFIX: &str = "profile:";

/// 오른쪽 클릭 메뉴. 프로필 하위 메뉴에 지금 프로필이 체크된다.
fn build_menu(app: &AppHandle) -> tauri::Result<Menu<tauri::Wry>> {
    let current = crate::profiles::current(app).id;
    let mut profile_items: Vec<Box<dyn tauri::menu::IsMenuItem<tauri::Wry>>> = Vec::new();
    for profile in crate::profiles::all(app) {
        let checked = profile.id == current;
        profile_items.push(Box::new(CheckMenuItem::with_id(app, format!("{PROFILE_PREFIX}{}", profile.id), profile.display_name(), true, checked, None::<&str>)?));
    }
    profile_items.push(Box::new(PredefinedMenuItem::separator(app)?));
    profile_items.push(Box::new(MenuItem::with_id(app, "profiles", tr!("프로필 관리…", "Manage Profiles…"), true, None::<&str>)?));
    let profile_refs: Vec<&dyn tauri::menu::IsMenuItem<tauri::Wry>> = profile_items.iter().map(|item| item.as_ref()).collect();
    let profiles = Submenu::with_items(app, tr!("프로필", "Profiles"), true, &profile_refs)?;
    Menu::with_items(
        app,
        &[
            &MenuItem::with_id(app, "manager", tr!("관리 창 열기", "Open Manager"), true, None::<&str>)?,
            &profiles,
            &MenuItem::with_id(app, "settings", tr!("설정…", "Settings…"), true, None::<&str>)?,
            &MenuItem::with_id(app, "welcome", tr!("안내", "Guide"), true, None::<&str>)?,
            &PredefinedMenuItem::separator(app)?,
            &MenuItem::with_id(app, "quit", tr!("LinkGroupz 종료", "Quit LinkGroupz"), true, None::<&str>)?,
        ],
    )
}

/// 프로필 목록 · 지금 프로필이 바뀌면 메뉴를 다시 만든다.
pub fn refresh_menu(app: &AppHandle) {
    let (Some(tray), Ok(menu)) = (app.tray_by_id(TRAY_ID), build_menu(app)) else {
        return;
    };
    let _ = tray.set_menu(Some(menu));
}
