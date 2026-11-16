//! LinkGroupz 앱: 메뉴바(트레이) 아이콘 · 관리 창 · 미리보기 · 자동 업데이트.
//!
//! 화면은 `ui/` 의 웹 페이지이고, 이 크레이트는 화면이 부르는 명령(`commands`)과 운영체제 쪽 일을 맡는다.
//! 화면과 무관한 규칙(모델 · 저장 · 검색 · 브라우저 연동)은 `linkgroupz-core` 에 있다.

mod commands;
mod favicon_store;
mod history;
mod launch;
mod preview;
#[cfg(target_os = "macos")]
mod preview_cookies;
mod private_watch;
mod profiles;
mod settings;
mod shortcut;
mod state;
mod tabs_host;
mod tray;
mod update;
mod windows;

use commands::WhatsNew;
use linkgroupz_core::{brand, AppVersion, BundledExtension, LinkStore, NativeSnapshotStore, ReleaseNotes};
use settings::Settings;
use state::{lock, AppState, StoreSnapshot, STORE_CHANGED};
use std::time::Duration;
use tauri::{AppHandle, Emitter, Manager, RunEvent};
use tauri_plugin_window_state::StateFlags;
use update::UpdateController;

pub fn run() {
    launch::replace_temporary_instance();

    let mut builder = tauri::Builder::default();
    // LinkGroupz 를 또 실행하면 새로 켠 쪽은 끝나고, 실행 중인 쪽이 관리 창을 열어 맨 앞으로 가져온다.
    // 여러 개가 실행되면 메뉴바 아이콘이 여러 개 생기고 서로의 변경을 덮어쓸 수 있다.
    // 예시 데이터로 띄운 경우(데이터 폴더 환경 변수)는 따로 실행되므로 확인하지 않는다.
    if std::env::var_os(brand::env::DATA_DIR).is_none() {
        builder = builder.plugin(tauri_plugin_single_instance::init(|app, _arguments, _directory| {
            windows::show_manager(app);
        }));
    }
    // ⌘Z · ⇧⌘Z 를 앱 되돌리기에도 쓰도록 편집 메뉴의 Undo · Redo 를 바꾼다 (history.rs).
    #[cfg(target_os = "macos")]
    {
        builder = builder.menu(history::edit_menu::app_menu).on_menu_event(history::edit_menu::on_menu_event);
    }
    #[cfg(windows)]
    {
        builder = builder.plugin(tauri_plugin_autostart::init(
            tauri_plugin_autostart::MacosLauncher::LaunchAgent,
            Some(vec![launch::AUTOSTART_ARGUMENT]),
        ));
    }

    builder
        .plugin(tauri_plugin_dialog::init())
        .plugin(tauri_plugin_opener::init())
        .plugin(tauri_plugin_clipboard_manager::init())
        .plugin(tauri_plugin_updater::Builder::new().build())
        .plugin(shortcut::plugin())
        .plugin(
            // 관리 창의 크기 · 위치만 기억한다.
            tauri_plugin_window_state::Builder::new()
                .with_state_flags(StateFlags::SIZE | StateFlags::POSITION | StateFlags::MAXIMIZED)
                .with_filter(|label| label == windows::MANAGER)
                .build(),
        )
        .invoke_handler(tauri::generate_handler![
            commands::bootstrap,
            commands::store_snapshot,
            commands::filter_groups,
            commands::filter_archive,
            commands::parse_url,
            commands::parse_tags,
            commands::add_group,
            commands::update_group,
            commands::set_group_browser,
            commands::set_group_always_private,
            commands::delete_groups,
            commands::move_groups,
            commands::move_items,
            commands::add_section,
            commands::rename_section,
            commands::set_section_collapsed,
            commands::delete_section,
            commands::move_section,
            commands::load_closed_tab_groups,
            commands::set_show_dock_icon,
            commands::set_show_manager_on_launch,
            commands::backups,
            commands::export_data,
            commands::choose_import_file,
            commands::import_data,
            commands::set_backup_days,
            commands::restore_backup,
            commands::archive_groups,
            commands::cleanup_group_icons,
            commands::add_link,
            commands::update_link,
            commands::add_links,
            commands::delete_links,
            commands::move_links_within,
            commands::move_links,
            commands::archive_links,
            commands::restore_groups,
            commands::restore_links,
            commands::restore_groups_to,
            commands::restore_archived_group_links,
            commands::delete_archived_group_links,
            commands::move_archived_links_into_group,
            commands::move_archived_groups,
            commands::move_archived_links,
            commands::move_links_in_archived_group,
            commands::archive_section,
            commands::open_archived_group,
            commands::open_archived_links,
            commands::restore_links_to_new_group,
            commands::delete_archived_groups,
            commands::delete_archived_links,
            commands::open_group,
            commands::open_group_private,
            commands::open_links,
            commands::filter_group_links,
            commands::set_search_scope,
            commands::set_link_opens_with_group,
            commands::browser_icon,
            commands::favicon,
            commands::group_icon_image,
            commands::choose_group_icon_image,
            commands::browser_states,
            commands::load_tabs,
            commands::load_tab_groups,
            commands::sync_group_plan,
            commands::sync_group_apply,
            commands::plan_tab_group_import,
            commands::import_tab_groups,
            commands::set_group_open_mode,
            commands::extension_states,
            commands::extension_folder,
            commands::reveal_extension_folder,
            commands::reveal_data_file,
            commands::copy_text,
            commands::launch_at_login,
            commands::set_launch_at_login,
            commands::open_login_items_settings,
            commands::toggle_preview,
            commands::close_preview,
            commands::previewing_link,
            commands::preview_info,
            commands::preview_back,
            commands::preview_forward,
            commands::preview_reload,
            commands::preview_expand,
            commands::preview_open_in_browser,
            commands::preview_site_count,
            commands::clear_preview_logins,
            commands::update_status,
            commands::update_set_auto_check,
            commands::update_check,
            commands::update_install,
            commands::update_later,
            commands::update_clear_message,
            commands::open_update_page,
            commands::show_manager,
            commands::show_settings,
            commands::show_welcome,
            commands::request_manager_action,
            commands::take_manager_action,
            commands::undo,
            commands::set_language,
            commands::set_theme,
            commands::redo,
            commands::native_edit,
            commands::show_whats_new,
            commands::whats_new_content,
            commands::open_guide,
            commands::open_licenses,
            commands::popover_shortcut,
            commands::set_popover_shortcut,
            commands::pause_popover_shortcut,
            commands::quit,
            commands::close_window,
            commands::fit_window_height,
            commands::resize_popover,
            commands::set_popover_size,
            commands::popover_layout,
            commands::set_popover_detached,
            commands::hide_popover,
            profiles::profiles,
            profiles::switch_profile,
            profiles::add_profile,
            profiles::update_profile,
            profiles::delete_profile,
        ])
        .setup(|app| {
            setup(app.handle())?;
            Ok(())
        })
        .build(tauri::generate_context!())
        .expect("LinkGroupz 를 시작하지 못했습니다")
        .run(|app, event| match event {
            // 창을 모두 닫아도 메뉴바에 남는다. 끝내는 것은 '종료'(exit 코드가 있는 요청)뿐이다.
            RunEvent::ExitRequested { api, code, .. } => {
                if code.is_none() {
                    api.prevent_exit();
                }
            }
            // 실행 중인 LinkGroupz 를 Finder 에서 다시 열거나 Dock 아이콘을 누르면 관리 창을 열어 앞으로 가져온다.
            #[cfg(target_os = "macos")]
            RunEvent::Reopen { .. } => windows::show_manager(app),
            _ => {
                let _ = app;
            }
        });
}

/// Dock 에 아이콘을 보일지 설정대로 맞춘다 (macOS). 꺼 두면 메뉴바 아이콘으로만 쓰는 앱이 된다.
pub(crate) fn apply_dock_icon(app: &AppHandle) {
    #[cfg(target_os = "macos")]
    {
        let show = lock(&app.state::<AppState>().settings).show_dock_icon;
        let policy = if show { tauri::ActivationPolicy::Regular } else { tauri::ActivationPolicy::Accessory };
        let _ = app.set_activation_policy(policy);
    }
    #[cfg(not(target_os = "macos"))]
    let _ = app;
}

fn setup(app: &AppHandle) -> tauri::Result<()> {
    // 처음에는 메뉴바 앱으로 뜬다 (Info.plist 의 LSUIElement). 설정을 읽은 뒤 Dock 아이콘을 맞춘다.
    #[cfg(target_os = "macos")]
    app.set_activation_policy(tauri::ActivationPolicy::Accessory)?;

    // 실행 이벤트를 처리하는 동안에만 알 수 있어 맨 먼저 확인한다.
    let launched_at_login = launch::is_login_item_launch();
    let arguments: Vec<String> = std::env::args().collect();
    let has_argument = |name: &str| arguments.iter().any(|argument| argument == name);

    let mut settings = Settings::load();
    // 화면 · 메뉴 · 알림 글의 언어 (트레이 메뉴 · 창 제목을 만들기 전에 정한다)
    linkgroupz_core::i18n::set_language(settings.resolved_language());
    // 화면 모드 (창을 만들기 전에 정한다). 시스템을 따르면 건드리지 않는다
    if let Some(theme) = settings.theme {
        app.set_theme(Some(theme.tauri_theme()));
    }
    let version = AppVersion::parse(&app.package_info().version.to_string()).unwrap_or_else(|| AppVersion::parse("0").expect("0 은 버전이다"));

    // 시작 안내가 '본 적 있음'을 기록하기 전에, 업데이트한 기존 사용자인지 판단해 바뀐 점을 정한다.
    let whats_new = whats_new_on_launch(&mut settings, &version, has_argument("--whats-new"));
    let show_welcome = has_argument("--welcome") || !settings.welcome_shown;
    if show_welcome && !settings.welcome_shown {
        settings.welcome_shown = true;
        settings.save();
    }

    let updater = UpdateController::new(version, &settings);
    app.manage(AppState::new(settings, updater));
    apply_dock_icon(app);

    // 다른 사람이 확장을 로드할 수 있도록 앱 안의 확장을 데이터 폴더로 복사해 둔다.
    BundledExtension::sync(commands::bundled_extension_dir(app).as_deref(), &BundledExtension::installed_dir());
    // 확장이 탭 목록을 앱에 자동 저장할 수 있도록 Native Messaging 호스트를 등록한다.
    NativeSnapshotStore::install_if_possible();
    // 30초 뒤, 그 뒤 1시간마다 새 버전이 올라왔는지 확인한다 (설정에서 끌 수 있다).
    app.state::<AppState>().updater.start(app);

    tray::create(app)?;
    run_backups(app);
    // 설정에서 정해 둔 단축키로 메뉴바 창을 여닫는다 (기본은 없음).
    shortcut::start(app);
    watch_store(app);
    // 사용법 스크린샷용: 메뉴바 창을 띄운 채 시작한다.
    if std::env::var(brand::env::SCREENSHOT_SCENE).as_deref() == Ok("menubar") {
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1500)).await;
            windows::show_popover_for_screenshot(&handle);
        });
    }

    // 직접 켰으면 정해 둔 창을 연다: 메뉴바(트레이) 창(기본) 또는 관리 창. 처음 켤 때는 관리 창과 시작 안내를 연다.
    // 재부팅 · 로그인 때 자동으로 켜진 경우는 아무 창도 열지 않는다.
    let open_manager = show_welcome || lock(&app.state::<AppState>().settings).show_manager_on_launch;
    let screenshot = std::env::var(brand::env::SCREENSHOT_SCENE).is_ok_and(|scene| !scene.is_empty());
    if !launched_at_login && open_manager {
        windows::show_manager(app);
    } else if !launched_at_login && whats_new.is_none() && !screenshot {
        // 메뉴바 아이콘이 자리를 잡은 뒤에 띄운다. ('바뀐 점' 창이 뜰 때는 그 창이 먼저다)
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(800)).await;
            let shown = handle.clone();
            let _ = handle.run_on_main_thread(move || windows::show_popover_at_tray(&shown));
        });
    }
    if let Some(content) = whats_new {
        *lock(&app.state::<AppState>().whats_new) = Some(content);
        // 관리 창이 먼저 뜨도록 조금 뒤에 띄운다.
        let handle = app.clone();
        tauri::async_runtime::spawn(async move {
            tokio::time::sleep(Duration::from_millis(1200)).await;
            windows::show_whats_new(&handle);
        });
    }
    if show_welcome {
        windows::show_welcome(app);
    }
    warn_if_running_from_disk_image(app);
    Ok(())
}

/// 켤 때 보여 줄 '바뀐 점'. 지난번 켠 버전보다 올라갔으면 그 사이 바뀐 점을 보여 주고, 지금 버전을 기록한다.
fn whats_new_on_launch(settings: &mut Settings, current: &AppVersion, forced: bool) -> Option<WhatsNew> {
    // 예시 데이터로 띄운 앱은 기록을 남기지 않는다.
    if std::env::var_os(brand::env::DATA_DIR).is_some() && !forced {
        return None;
    }
    let last_seen = settings.last_seen_version.as_deref().and_then(AppVersion::parse);
    let entries = if forced {
        ReleaseNotes::entries_after(None, current, ReleaseNotes::entries())
    } else {
        ReleaseNotes::entries_to_show(last_seen.as_ref(), settings.welcome_shown, current, ReleaseNotes::entries())
    };
    if !forced && last_seen.as_ref().is_none_or(|seen| seen < current) {
        settings.last_seen_version = Some(current.to_string());
        settings.save();
    }
    if entries.is_empty() {
        return None;
    }
    Some(WhatsNew { title: linkgroupz_core::tr!("LinkGroupz {} 으로 업데이트했습니다", "Updated to LinkGroupz {}", current), entries: entries.iter().map(linkgroupz_core::ReleaseEntry::localized).collect() })
}

/// 앱 밖에서 data.json 이 바뀌면(직접 수정, 백업 복원) 다시 읽어 모든 창에 알린다.
///
/// 저장은 임시 파일을 만든 뒤 바꿔치기하므로, 파일의 수정 시각 · 크기가 달라졌을 때만 내용을 비교한다.
/// 켠 뒤 잠시 뒤, 그 뒤 1시간마다 그날의 백업을 만들고 보관 기간이 지난 백업을 지운다 (crates/core/src/backups.rs).
fn run_backups(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || loop {
        std::thread::sleep(Duration::from_secs(20));
        run_backups_now(&app);
        std::thread::sleep(Duration::from_secs(60 * 60));
    });
}

/// 모든 프로필의 오늘 백업을 만들고 보관 기간이 지난 백업을 지운다. 백업은 프로필 폴더마다 따로 있다.
pub(crate) fn run_backups_now(app: &AppHandle) {
    let (keep_days, profiles) = {
        let state = app.state::<AppState>();
        let settings = lock(&state.settings);
        (settings.backup_days, settings.profiles())
    };
    let data_directory = LinkStore::data_directory();
    for profile in profiles {
        linkgroupz_core::backups::Backups::run(&profile.data_file(&data_directory), &profile.backup_directory(&data_directory), keep_days, &data_directory);
    }
}

fn watch_store(app: &AppHandle) {
    let app = app.clone();
    std::thread::spawn(move || {
        let current_file = || lock(&app.state::<AppState>().store).file_path().to_owned();
        let stamp = |path: &std::path::Path| std::fs::metadata(path).ok().map(|metadata| (metadata.modified().ok(), metadata.len()));
        let mut file = current_file();
        let mut last = stamp(&file);
        loop {
            std::thread::sleep(Duration::from_millis(1500));
            // 프로필을 바꾸면 그 프로필의 파일을 지켜본다.
            let now_file = current_file();
            if now_file != file {
                file = now_file;
                last = stamp(&file);
                continue;
            }
            let now = stamp(&file);
            if now == last {
                continue;
            }
            last = now;
            let state = app.state::<AppState>();
            let snapshot = {
                let mut store = lock(&state.store);
                let reloaded = store.reload_if_changed_on_disk();
                if reloaded {
                    // 밖에서 바뀐 내용을 되돌리기로 덮어쓰지 않도록 기록을 비운다.
                    state.history.forget(store.file_path());
                }
                reloaded.then(|| StoreSnapshot::of(&store))
            };
            if let Some(snapshot) = snapshot {
                let _ = app.emit(STORE_CHANGED, snapshot);
            }
        }
    });
}

/// DMG(디스크 이미지) 안에서 바로 실행한 경우: DMG 를 꺼내면 앱이 사라지므로 응용 프로그램 폴더로 옮기라고 안내한다.
#[cfg(target_os = "macos")]
fn warn_if_running_from_disk_image(app: &AppHandle) {
    use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
    use tauri_plugin_opener::OpenerExt;

    let from_disk_image = launch::bundle_path().is_some_and(|path| launch::is_temporary_location(&path.to_string_lossy()));
    if !from_disk_image {
        return;
    }
    let handle = app.clone();
    app.dialog()
        .message(linkgroupz_core::tr!("LinkGroupz 를 응용 프로그램 폴더로 끌어다 놓은 뒤, 응용 프로그램 폴더에서 실행해 주세요.\nDMG 안에서 실행하면 DMG 를 꺼낼 때 앱이 사라집니다.", "Drag LinkGroupz to the Applications folder and open it from there.\nIf you run it from the DMG, the app disappears when the DMG is ejected."))
        .title(linkgroupz_core::tr!("디스크 이미지(DMG) 안에서 실행 중입니다", "Running from the disk image (DMG)"))
        .kind(MessageDialogKind::Warning)
        .buttons(MessageDialogButtons::OkCancelCustom(linkgroupz_core::tr!("응용 프로그램 폴더 열기", "Open Applications Folder").to_owned(), linkgroupz_core::tr!("그대로 실행", "Keep Running").to_owned()))
        .show(move |open_applications| {
            if open_applications {
                let _ = handle.opener().open_path("/Applications", None::<&str>);
                handle.exit(0);
            }
        });
}

#[cfg(not(target_os = "macos"))]
fn warn_if_running_from_disk_image(_app: &AppHandle) {}
