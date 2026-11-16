//! 화면(웹뷰)이 부르는 명령들.
//!
//! 브라우저 실행 · 키체인 · 네트워크 · 창 만들기처럼 기다릴 수 있는 일은 모두 `async` 명령으로 두어
//! 메인 스레드(화면)가 멈추지 않게 한다.

use crate::favicon_store::data_url;
use crate::launch::{self, LaunchAtLoginState};
use crate::preview::{self, Anchor, PreviewInfo, PreviewRequest};
use crate::history;
use linkgroupz_core::tr;
use crate::state::{edit_store, lock, mutate_store, read_store, AppState, StoreSnapshot};
use crate::tabs_host::AppTabsHost;
use crate::private_watch;
use crate::shortcut::{self, ShortcutStatus};
use crate::update::UpdateStatus;
use crate::windows;
use linkgroupz_core::{
    brand, Browser, BundledExtension, ChromeExtensionDetector, ChromeLauncher, ChromeTabGroupImport, ChromeTabSource,
    FoundExtension, GroupColor, GroupIcon, GroupIconStore, GroupOpenMode, Id, LauncherError, LinkArchive, LinkGroup,
    LinkStore, LinkUrl, MoveResult, NativeSnapshotStore, ReleaseEntry, ReleaseNotes, SavedLink, SearchFilter,
    TabGroupImportPlan, TabImportCandidate, Timestamp, DropTarget,
};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::path::{Path, PathBuf};
use tauri::{AppHandle, Manager, State, Webview, WebviewWindow};
use tauri_plugin_clipboard_manager::ClipboardExt;
use tauri_plugin_dialog::{DialogExt, MessageDialogButtons, MessageDialogKind};
use tauri_plugin_opener::OpenerExt;

type CommandResult<T = ()> = Result<T, String>;

fn id_set(ids: Vec<Id>) -> HashSet<Id> {
    ids.into_iter().collect()
}

/// 홈 폴더를 `~` 로 줄인 경로 (화면 표시용)
fn display_path(path: &Path) -> String {
    match dirs_home().and_then(|home| path.strip_prefix(home).ok().map(Path::to_owned)) {
        Some(rest) if cfg!(not(windows)) => format!("~/{}", rest.display()),
        _ => path.display().to_string(),
    }
}

fn dirs_home() -> Option<PathBuf> {
    std::env::var_os(if cfg!(windows) { "USERPROFILE" } else { "HOME" }).map(PathBuf::from)
}

/// 기다리는 일을 백그라운드 스레드에서 한다.
async fn blocking<T: Send + 'static>(work: impl FnOnce() -> T + Send + 'static) -> CommandResult<T> {
    tauri::async_runtime::spawn_blocking(work).await.map_err(|error| error.to_string())
}

// MARK: - 시작할 때 한 번 받는 내용

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ColorInfo {
    id: &'static str,
    name: &'static str,
    chrome_id: &'static str,
    chrome_name: &'static str,
    is_chrome: bool,
    rgb: [u8; 3],
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BrowserInfo {
    id: Browser,
    name: &'static str,
    full_name: &'static str,
    installed: bool,
    extensions_page: &'static str,
    load_unpacked_label: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct OpenModeInfo {
    id: GroupOpenMode,
    name: &'static str,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct Bootstrap {
    app_name: &'static str,
    version: String,
    /// "macos" · "windows" · "linux"
    platform: &'static str,
    colors: Vec<ColorInfo>,
    preset_symbols: Vec<&'static str>,
    default_symbol: &'static str,
    browsers: Vec<BrowserInfo>,
    open_modes: Vec<OpenModeInfo>,
    group_open_mode: GroupOpenMode,
    drag_prefix: &'static str,
    data_file: String,
    /// 사용법 스크린샷용 화면 상태
    scene: Option<String>,
    has_guide: bool,
    has_licenses: bool,
    show_dock_icon: bool,
    show_manager_on_launch: bool,
    /// 프로필 목록 · 지금 프로필 (바뀌면 profiles-changed 로 다시 알린다)
    profiles: crate::profiles::ProfileState,
    /// 검색할 곳 (바뀌면 search-scope-changed 로 다시 알린다)
    search_scope: linkgroupz_core::SearchScope,
    /// 지금 화면 언어 ("ko" · "en")
    language: &'static str,
    /// 설정에서 고른 언어 (없으면 시스템을 따른다)
    language_setting: Option<linkgroupz_core::i18n::Language>,
    /// 설정에서 고른 화면 모드 (없으면 시스템을 따른다)
    theme_setting: Option<crate::settings::Appearance>,
}

fn browser_infos() -> Vec<BrowserInfo> {
    Browser::ALL
        .into_iter()
        .map(|browser| BrowserInfo {
            id: browser,
            name: browser.display_name(),
            full_name: browser.full_name(),
            installed: browser.is_installed(),
            extensions_page: browser.extensions_page(),
            load_unpacked_label: browser.load_unpacked_label(),
        })
        .collect()
}

#[tauri::command(async)]
pub fn bootstrap(app: AppHandle, state: State<'_, AppState>) -> Bootstrap {
    // 설정은 한 번만 잠근다 (한 식 안에서 두 번 잠그면 앞의 잠금이 식이 끝날 때까지 남아 멈춘다).
    let (group_open_mode, show_dock_icon, show_manager_on_launch, search_scope, language_setting, theme_setting) = {
        let settings = lock(&state.settings);
        (settings.group_open_mode, settings.show_dock_icon, settings.show_manager_on_launch, settings.search_scope, settings.language, settings.theme)
    };
    Bootstrap {
        app_name: brand::APP_NAME,
        version: state.updater.current_version().to_string(),
        platform: if cfg!(target_os = "macos") {
            "macos"
        } else if cfg!(windows) {
            "windows"
        } else {
            "linux"
        },
        colors: GroupColor::ALL
            .into_iter()
            .map(|color| {
                let (red, green, blue) = color.rgb();
                ColorInfo {
                    id: color.raw_value(),
                    name: color.display_name(),
                    chrome_id: color.chrome_color().raw_value(),
                    chrome_name: color.chrome_color().display_name(),
                    is_chrome: color.is_chrome_color(),
                    rgb: [red, green, blue],
                }
            })
            .collect(),
        preset_symbols: GroupIcon::PRESET_SYMBOLS.to_vec(),
        default_symbol: GroupIcon::DEFAULT_SYMBOL,
        browsers: browser_infos(),
        open_modes: GroupOpenMode::ALL.into_iter().map(|mode| OpenModeInfo { id: mode, name: mode.display_name() }).collect(),
        group_open_mode,
        drag_prefix: brand::DRAG_PAYLOAD_PREFIX,
        data_file: read_store(&app, |store| display_path(store.file_path())),
        profiles: crate::profiles::state(&app),
        search_scope,
        show_dock_icon,
        show_manager_on_launch,
        scene: std::env::var(brand::env::SCREENSHOT_SCENE).ok().filter(|scene| !scene.is_empty()),
        has_guide: guide_file(&app).is_some(),
        has_licenses: licenses_file(&app).is_some(),
        language: linkgroupz_core::i18n::language().code(),
        language_setting,
        theme_setting,
    }
}

// MARK: - 그룹 · 페이지 조회

#[tauri::command]
pub fn store_snapshot(app: AppHandle) -> StoreSnapshot {
    read_store(&app, StoreSnapshot::of)
}

/// 설정의 검색할 곳을 붙인 검색 조건
fn search_filter(app: &AppHandle, query: &str) -> SearchFilter {
    let scope = lock(&app.state::<AppState>().settings).search_scope;
    SearchFilter::new(query).with_scope(scope)
}

/// 검색어에 맞는 그룹 (맞는 페이지만 담아서). 검색어가 비면 전체.
#[tauri::command]
pub fn filter_groups(app: AppHandle, query: String) -> Vec<LinkGroup> {
    let filter = search_filter(&app, &query);
    read_store(&app, |store| filter.apply(store.groups()))
}

#[tauri::command]
pub fn filter_archive(app: AppHandle, query: String) -> LinkArchive {
    let filter = search_filter(&app, &query);
    read_store(&app, |store| store.archive().filtered(&filter))
}

/// 그룹 안 검색: 검색어에 맞는 페이지 id (그룹 화면의 '이 그룹에서 찾기')
#[tauri::command]
pub fn filter_group_links(app: AppHandle, query: String, group_id: Id) -> Vec<Id> {
    let filter = search_filter(&app, &query);
    read_store(&app, |store| {
        store.group(group_id).map(|group| group.links.iter().filter(|link| filter.matches_link(link)).map(|link| link.id).collect()).unwrap_or_default()
    })
}

pub const SEARCH_SCOPE_CHANGED: &str = "search-scope-changed";

pub const LANGUAGE_CHANGED: &str = "language-changed";

/// 화면 언어를 바꾼다 (None 이면 시스템 언어). 모든 창이 다시 그리고, 트레이 메뉴 · 창 제목도 바꾼다.
#[tauri::command]
pub fn set_language(app: AppHandle, language: Option<linkgroupz_core::i18n::Language>) {
    let resolved = {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        settings.language = language;
        settings.save();
        settings.resolved_language()
    };
    linkgroupz_core::i18n::set_language(resolved);
    crate::tray::refresh_menu(&app);
    crate::windows::retitle_windows(&app);
    let _ = tauri::Emitter::emit(&app, LANGUAGE_CHANGED, resolved.code());
}

/// 화면 모드를 바꾼다 (None 이면 시스템을 따른다). 앱 전체 모양이 바뀌어 모든 창 · 메뉴가 함께 바뀐다.
#[tauri::command]
pub fn set_theme(app: AppHandle, theme: Option<crate::settings::Appearance>) {
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        settings.theme = theme;
        settings.save();
    }
    app.set_theme(theme.map(crate::settings::Appearance::tauri_theme));
}

/// 검색할 곳을 바꾼다. 모든 창에 알린다.
#[tauri::command]
pub fn set_search_scope(app: AppHandle, scope: linkgroupz_core::SearchScope) {
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        settings.search_scope = scope;
        settings.save();
    }
    let _ = tauri::Emitter::emit(&app, SEARCH_SCOPE_CHANGED, scope);
}

/// 입력한 글자를 주소로 바꾼다. 주소가 아니면 None.
#[tauri::command]
pub fn parse_url(text: String) -> Option<LinkUrl> {
    SavedLink::parse_url(&text)
}

#[tauri::command]
pub fn parse_tags(text: String) -> Vec<String> {
    LinkGroup::parse_tags(&text)
}

// MARK: - 그룹

/// 그룹 편집 창이 보내는 값 (페이지 목록은 따로 다룬다)
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct GroupInput {
    name: String,
    color: GroupColor,
    tags: Vec<String>,
    browser: Browser,
    icon: GroupIcon,
    always_private: bool,
    /// 들어갈 섹션. 보내지 않으면 그대로 두고, null 이면 섹션 밖으로
    #[serde(default, rename = "sectionID", deserialize_with = "present")]
    section_id: Option<Option<Id>>,
}

/// 보낸 값이 null 이어도 '보냈음'으로 읽는다 (보내지 않았으면 serde 의 기본값 None).
fn present<'de, D: serde::Deserializer<'de>>(deserializer: D) -> Result<Option<Option<Id>>, D::Error> {
    Option::<Id>::deserialize(deserializer).map(Some)
}

impl GroupInput {
    fn apply(self, group: &mut LinkGroup) -> CommandResult {
        let name = self.name.trim();
        if name.is_empty() {
            return Err(tr!("그룹 이름을 입력해 주세요.", "Enter a group name.").to_owned());
        }
        group.name = name.to_owned();
        group.color = self.color;
        group.tags = LinkGroup::normalized_tags(&self.tags);
        group.browser = self.browser;
        group.icon = self.icon;
        group.always_private = self.always_private;
        if let Some(section_id) = self.section_id {
            group.section_id = section_id;
        }
        Ok(())
    }
}

/// 어느 그룹도 쓰지 않는 아이콘 이미지(아이콘을 바꿨거나 그룹을 지웠거나 편집을 취소한 경우)를 지운다.
/// 아이콘 폴더는 모든 프로필의 그룹 · 프로필 아이콘이 함께 쓰므로 모두 본다. 읽지 못한 프로필이 있으면 지우지 않는다.
pub(crate) fn remove_unused_icons(app: &AppHandle) {
    let Some(groups) = crate::profiles::groups_in_all_profiles(app) else {
        return;
    };
    let profiles = crate::profiles::all(app);
    // 되돌리기 기록에 든 그룹의 이미지도 남긴다 (되돌리면 다시 쓴다).
    let in_history = app.state::<AppState>().history.icon_file_names();
    let used: HashSet<&str> = groups
        .iter()
        .filter_map(|group| group.icon.image_file_name())
        .chain(profiles.iter().filter_map(|profile| profile.icon.image_file_name()))
        .chain(in_history.iter().map(String::as_str))
        .collect();
    GroupIconStore::remove_unused_names(&used, &LinkStore::data_directory());
}

#[tauri::command]
pub fn add_group(app: AppHandle, input: GroupInput) -> CommandResult<Id> {
    let mut group = LinkGroup::new("");
    input.apply(&mut group)?;
    let label = tr!("‘{}’ 그룹 추가", "Add group “{}”", group.name);
    let id = edit_store(&app, |_| label, |store| store.add_group(group)).id;
    remove_unused_icons(&app);
    Ok(id)
}

#[tauri::command]
pub fn update_group(app: AppHandle, id: Id, input: GroupInput) -> CommandResult {
    let mut group = read_store(&app, |store| store.group(id).cloned()).ok_or(tr!("그룹을 찾지 못했습니다.", "Couldn't find the group."))?;
    input.apply(&mut group)?;
    edit_store(&app, |store| tr!("{} 편집", "Edit {}", history::groups(store, &[id])), |store| store.update_group_keeping_place(group));
    remove_unused_icons(&app);
    Ok(())
}

fn change_group(app: &AppHandle, id: Id, what: &str, change: impl FnOnce(&mut LinkGroup)) {
    edit_store(app, |store| format!("{} {what}", history::groups(store, &[id])), |store| {
        if let Some(mut group) = store.group(id).cloned() {
            change(&mut group);
            store.update_group(group);
        }
    });
}

#[tauri::command]
pub fn set_group_browser(app: AppHandle, id: Id, browser: Browser) {
    change_group(&app, id, tr!("브라우저 바꾸기", "Change browser"), |group| group.browser = browser);
}

#[tauri::command]
pub fn set_group_always_private(app: AppHandle, id: Id, always_private: bool) {
    change_group(&app, id, if always_private { tr!("항상 시크릿 모드 켜기", "Turn on always private") } else { tr!("항상 시크릿 모드 끄기", "Turn off always private") }, |group| group.always_private = always_private);
}

#[tauri::command]
pub fn delete_groups(app: AppHandle, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 삭제", "Delete {}", history::groups(store, &ids)), |store| store.delete_groups(&id_set(ids.clone())));
    remove_unused_icons(&app);
}

#[tauri::command]
pub fn move_groups(app: AppHandle, from: Vec<usize>, to: usize) {
    edit_store(&app, |_| tr!("그룹 순서 바꾸기", "Reorder groups").to_owned(), |store| store.move_groups(&from, to));
}

/// 그룹 · 섹션을 끌어다 놓는다 (여러 개면 화면 순서대로).
#[tauri::command]
pub fn move_items(app: AppHandle, ids: Vec<Id>, target: DropTarget) {
    edit_store(&app, |_| tr!("그룹 · 섹션 옮기기", "Move groups and sections").to_owned(), |store| store.move_items(&ids, target));
}

// MARK: - 섹션

/// 섹션을 만든다. at(섹션 · 그룹)의 자리에 놓고 group_ids 의 그룹을 넣는다. 만든 섹션의 id 를 돌려준다.
#[tauri::command]
pub fn add_section(app: AppHandle, name: String, at: Option<Id>, group_ids: Vec<Id>) -> CommandResult<Id> {
    if name.trim().is_empty() {
        return Err(tr!("섹션 이름을 입력해 주세요.", "Enter a section name.").to_owned());
    }
    Ok(edit_store(&app, |_| tr!("‘{}’ 섹션 추가", "Add section “{}”", name.trim()), |store| store.add_section(&name, at, &group_ids)).id)
}

#[tauri::command]
pub fn rename_section(app: AppHandle, id: Id, name: String) -> CommandResult {
    if name.trim().is_empty() {
        return Err(tr!("섹션 이름을 입력해 주세요.", "Enter a section name.").to_owned());
    }
    edit_store(&app, |store| tr!("{} 이름 바꾸기", "Rename {}", history::section(store, id)), |store| store.rename_section(id, &name));
    Ok(())
}

#[tauri::command]
pub fn set_section_collapsed(app: AppHandle, id: Id, collapsed: bool) {
    mutate_store(&app, |store| store.set_section_collapsed(id, collapsed));
}

/// 섹션을 지운다. 안의 그룹은 섹션이 있던 자리에 남는다.
#[tauri::command]
pub fn delete_section(app: AppHandle, id: Id) {
    edit_store(&app, |store| tr!("{} 삭제", "Delete {}", history::section(store, id)), |store| store.delete_section(id));
}

/// 섹션을 한 칸 위(-1) · 아래(+1)로
#[tauri::command]
pub fn move_section(app: AppHandle, id: Id, offset: isize) {
    edit_store(&app, |store| tr!("{} 옮기기", "Move {}", history::section(store, id)), |store| store.move_section(id, offset));
}

#[tauri::command]
pub fn archive_groups(app: AppHandle, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 보관", "Archive {}", history::groups(store, &ids)), |store| store.archive_groups(&id_set(ids.clone()), Timestamp::now()));
}

/// 그룹 편집을 취소했을 때: 고르다 만 아이콘 이미지를 정리한다.
#[tauri::command]
pub fn cleanup_group_icons(app: AppHandle) {
    remove_unused_icons(&app);
}

// MARK: - 페이지

/// 가져오기 화면이 보내는 페이지 (주소는 브라우저가 알려 준 그대로)
#[derive(Deserialize)]
pub struct NewLink {
    url: String,
    alias: Option<String>,
}

fn require_url(text: &str) -> CommandResult<LinkUrl> {
    SavedLink::parse_url(text).ok_or_else(|| tr!("http(s) URL 형식이 아닙니다.", "This isn't an http(s) URL.").to_owned())
}

#[tauri::command]
pub fn add_link(app: AppHandle, group_id: Id, url: String, alias: Option<String>) -> CommandResult {
    let link = SavedLink::new(require_url(&url)?, alias.as_deref());
    let label = tr!("‘{}’ 추가", "Add “{}”", link.display_name());
    edit_store(&app, |_| label, |store| store.add_link(link, group_id));
    Ok(())
}

#[tauri::command]
pub fn update_link(app: AppHandle, group_id: Id, link_id: Id, url: String, alias: Option<String>) -> CommandResult {
    let url = require_url(&url)?;
    edit_store(&app, |store| tr!("{} 편집", "Edit {}", history::links(store, group_id, &[link_id])), |store| {
        let opens_with_group = store.group(group_id).and_then(|group| group.links.iter().find(|link| link.id == link_id)).is_none_or(|link| link.opens_with_group);
        store.update_link(SavedLink { id: link_id, url, alias: SavedLink::normalized_alias(alias.as_deref()), opens_with_group }, group_id)
    });
    Ok(())
}

#[tauri::command]
pub fn add_links(app: AppHandle, group_id: Id, links: Vec<NewLink>) {
    let links: Vec<SavedLink> = links.into_iter().map(|link| SavedLink::new(LinkUrl::new(link.url), link.alias.as_deref())).collect();
    let label = tr!("페이지 {}개 추가", "Add {} pages", links.len());
    edit_store(&app, |_| label, |store| store.add_links(links, group_id));
}

#[tauri::command]
pub fn delete_links(app: AppHandle, group_id: Id, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 삭제", "Delete {}", history::links(store, group_id, &ids)), |store| store.delete_links(&id_set(ids.clone()), group_id));
}

#[tauri::command]
pub fn move_links_within(app: AppHandle, group_id: Id, from: Vec<usize>, to: usize) {
    edit_store(&app, |_| tr!("페이지 순서 바꾸기", "Reorder pages").to_owned(), |store| store.move_links_within(group_id, &from, to));
}

#[tauri::command]
pub fn move_links(app: AppHandle, ids: Vec<Id>, source_id: Id, target_id: Id) -> MoveResult {
    edit_store(
        &app,
        |store| tr!("{} → {} 옮기기", "Move {} → {}", history::links(store, source_id, &ids), history::groups(store, &[target_id])),
        |store| store.move_links(&id_set(ids.clone()), source_id, target_id),
    )
}

#[tauri::command]
pub fn archive_links(app: AppHandle, group_id: Id, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 보관", "Archive {}", history::links(store, group_id, &ids)), |store| store.archive_links(&id_set(ids.clone()), group_id, Timestamp::now()));
}

// MARK: - 보관함

#[tauri::command]
pub fn restore_groups(app: AppHandle, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 복원", "Restore {}", history::archived_groups(store, &ids)), |store| store.restore_groups(&id_set(ids.clone())));
}

#[tauri::command]
pub fn restore_links(app: AppHandle, ids: Vec<Id>, target_id: Id) -> MoveResult {
    edit_store(&app, |store| tr!("{} 복원", "Restore {}", history::archived_links(store, &ids)), |store| store.restore_links(&id_set(ids.clone()), target_id))
}

// MARK: - 보관함: 끌어 놓기 · 보관 그룹 안 페이지 · 바로 열기

/// 보관한 그룹을 그룹 목록의 그 자리로 되돌린다 (보관함에서 왼쪽 그룹 목록으로 끌어 놓기).
#[tauri::command]
pub fn restore_groups_to(app: AppHandle, ids: Vec<Id>, target: DropTarget) {
    edit_store(&app, |store| tr!("{} 복원", "Restore {}", history::archived_groups(store, &ids)), |store| store.restore_groups_to(&ids, target));
}

/// 보관한 그룹 안의 페이지를 그룹으로 되돌린다.
#[tauri::command]
pub fn restore_archived_group_links(app: AppHandle, group_id: Id, ids: Vec<Id>, target_id: Id) -> MoveResult {
    edit_store(&app, |_| tr!("보관 그룹의 페이지 {}개 복원", "Restore {} pages from an archived group", ids.len()), |store| store.restore_links_from_archived_group(group_id, &id_set(ids.clone()), target_id))
}

/// 보관한 그룹 안의 페이지를 지운다.
#[tauri::command]
pub fn delete_archived_group_links(app: AppHandle, group_id: Id, ids: Vec<Id>) {
    edit_store(&app, |_| tr!("보관 그룹의 페이지 {}개 영구 삭제", "Permanently delete {} pages in an archived group", ids.len()), |store| store.delete_links_in_archived_group(group_id, &id_set(ids.clone())));
}

/// 보관한 페이지를 보관한 그룹 안으로 넣는다.
#[tauri::command]
pub fn move_archived_links_into_group(app: AppHandle, ids: Vec<Id>, group_id: Id) {
    edit_store(&app, |store| tr!("보관 그룹에 {} 넣기", "Put {} into an archived group", history::archived_links(store, &ids)), |store| store.move_archived_links_into_group(&id_set(ids.clone()), group_id));
}

#[tauri::command]
pub fn move_archived_groups(app: AppHandle, from: Vec<usize>, to: usize) {
    edit_store(&app, |_| tr!("보관함 순서 바꾸기", "Reorder the archive").to_owned(), |store| store.move_archived_groups(&from, to));
}

#[tauri::command]
pub fn move_archived_links(app: AppHandle, from: Vec<usize>, to: usize) {
    edit_store(&app, |_| tr!("보관함 순서 바꾸기", "Reorder the archive").to_owned(), |store| store.move_archived_links(&from, to));
}

#[tauri::command]
pub fn move_links_in_archived_group(app: AppHandle, group_id: Id, from: Vec<usize>, to: usize) {
    edit_store(&app, |_| tr!("보관함 순서 바꾸기", "Reorder the archive").to_owned(), |store| store.move_links_in_archived_group(group_id, &from, to));
}

/// 섹션을 보관한다: 안의 그룹을 보관하고 섹션은 지운다 (보관함 버튼에 섹션을 끌어 놓기).
#[tauri::command]
pub fn archive_section(app: AppHandle, id: Id) {
    edit_store(&app, |store| tr!("{} 보관", "Archive {}", history::section(store, id)), |store| store.archive_section(id, Timestamp::now()));
    remove_unused_icons(&app);
}

/// 보관한 그룹을 바로 연다 (보관함에서 더블클릭). 스위치를 끈 페이지는 열지 않는다.
#[tauri::command(async)]
pub fn open_archived_group(app: AppHandle, window: WebviewWindow, id: Id) {
    let Some(group) = read_store(&app, |store| store.archive().groups.iter().find(|item| item.id() == id).map(|item| item.group.clone())) else {
        return;
    };
    report_open_failure(&app, open_group_now(&app, &window, &group, false));
}

/// 보관한 페이지를 바로 연다 (보관함에서 더블클릭). group_id 가 있으면 보관한 그룹 안의 페이지.
/// 보관 페이지는 보관하기 전 그룹이 아직 있으면 그 그룹의 브라우저로, 없으면 Chrome 으로 연다.
#[tauri::command(async)]
pub fn open_archived_links(app: AppHandle, ids: Vec<Id>, group_id: Option<Id>) {
    let ids = id_set(ids);
    let (links, browser) = read_store(&app, |store| match group_id {
        Some(group_id) => store
            .archive()
            .groups
            .iter()
            .find(|item| item.id() == group_id)
            .map(|item| (item.group.links.iter().filter(|link| ids.contains(&link.id)).cloned().collect::<Vec<_>>(), item.group.browser))
            .unwrap_or_default(),
        None => {
            let items: Vec<_> = store.archive().links.iter().filter(|item| ids.contains(&item.id())).collect();
            let browser = items.first().and_then(|item| store.group(item.group_id)).map_or(Browser::Chrome, |group| group.browser);
            (items.into_iter().map(|item| item.link.clone()).collect(), browser)
        }
    });
    report_open_failure(&app, links.iter().try_for_each(|link| ChromeLauncher::open_link(link, browser)));
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoredToNewGroup {
    #[serde(rename = "groupID")]
    group_id: Id,
    result: MoveResult,
}

#[tauri::command]
pub fn restore_links_to_new_group(app: AppHandle, ids: Vec<Id>, input: GroupInput) -> CommandResult<RestoredToNewGroup> {
    let mut group = LinkGroup::new("");
    input.apply(&mut group)?;
    let group_id = group.id;
    let label = tr!("보관 페이지를 새 그룹 ‘{}’ 에 복원", "Restore archived pages to the new group “{}”", group.name);
    let result = edit_store(&app, |_| label, |store| store.restore_links_to_new_group(&id_set(ids), group));
    remove_unused_icons(&app);
    Ok(RestoredToNewGroup { group_id, result })
}

#[tauri::command]
pub fn delete_archived_groups(app: AppHandle, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 영구 삭제", "Permanently delete {}", history::archived_groups(store, &ids)), |store| store.delete_archived_groups(&id_set(ids.clone())));
    remove_unused_icons(&app);
}

#[tauri::command]
pub fn delete_archived_links(app: AppHandle, ids: Vec<Id>) {
    edit_store(&app, |store| tr!("{} 영구 삭제", "Permanently delete {}", history::archived_links(store, &ids)), |store| store.delete_archived_links(&id_set(ids.clone())));
}

// MARK: - 열기

/// 화면 어디서 열든 같은 오류 처리를 쓴다: 열지 못하면 알림 창을 띄운다.
fn report_open_failure(app: &AppHandle, result: Result<(), LauncherError>) {
    if let Err(error) = result {
        app.dialog()
            .message(error.to_string())
            .title(tr!("열지 못했습니다", "Couldn't open"))
            .kind(MessageDialogKind::Warning)
            .buttons(MessageDialogButtons::OkCustom(tr!("확인", "OK").to_owned()))
            .show(|_| {});
    }
}

/// 시크릿 창으로 연다. 관리 창에서 연 것이면 그 창을 닫을 때 관리 창으로 돌아온다 (macOS).
fn open_private_from(app: &AppHandle, window: &WebviewWindow, links: &[SavedLink], browser: Browser) -> Result<(), LauncherError> {
    let from_manager = window.label() == windows::MANAGER;
    private_watch::open_watching(app, browser, from_manager, || ChromeLauncher::open_private(links, browser))
}

/// 확장을 설치 · 제거 · 끄기 한 상태가 바로 반영되도록 열 때마다 그룹의 브라우저에서 확인한다.
fn uses_tab_group(app: &AppHandle, group: &LinkGroup) -> bool {
    let mode = lock(&app.state::<AppState>().settings).group_open_mode;
    mode.uses_tab_group(|| ChromeExtensionDetector::find(group.browser).is_some())
}

/// 그룹의 모든 페이지 스위치가 꺼져 있으면 알린다 (열 것이 없다).
fn report_nothing_to_open(app: &AppHandle, group: &LinkGroup) {
    app.dialog()
        .message(tr!("‘{}’ 그룹의 페이지가 모두 꺼져 있습니다. 그룹을 열 때 열 페이지는 페이지 줄의 스위치를 켜 주세요.", "All pages in “{}” are turned off. Turn on the switch of the pages to open with the group.", group.name))
        .title(tr!("열 페이지가 없습니다", "No pages to open"))
        .kind(MessageDialogKind::Info)
        .buttons(MessageDialogButtons::OkCustom(tr!("확인", "OK").to_owned()))
        .show(|_| {});
}

/// 그룹을 연다. 스위치를 끈 페이지는 열지 않는다.
fn open_group_now(app: &AppHandle, window: &WebviewWindow, group: &LinkGroup, new_window: bool) -> Result<(), LauncherError> {
    let group = &group.for_opening();
    if group.links.is_empty() {
        report_nothing_to_open(app, group);
        return Ok(());
    }
    if group.always_private {
        return open_private_from(app, window, &group.links, group.browser);
    }
    let tab_group = uses_tab_group(app, group);
    if new_window {
        ChromeLauncher::open_group_in_new_window(group, tab_group)
    } else {
        ChromeLauncher::open_group(group, tab_group)
    }
}

/// new_window: 브라우저가 이미 떠 있어도 지금 창에 더하지 않고 새 창에 연다.
#[tauri::command(async)]
pub fn open_group(app: AppHandle, window: WebviewWindow, id: Id, new_window: Option<bool>) {
    let Some(group) = read_store(&app, |store| store.group(id).cloned()) else {
        return;
    };
    report_open_failure(&app, open_group_now(&app, &window, &group, new_window.unwrap_or(false)));
}

/// 그룹의 페이지를 그룹 브라우저의 시크릿 창(Edge 는 InPrivate) 하나에 모두 연다.
#[tauri::command(async)]
pub fn open_group_private(app: AppHandle, window: WebviewWindow, id: Id) {
    let Some(group) = read_store(&app, |store| store.group(id).cloned()) else {
        return;
    };
    let group = group.for_opening();
    if group.links.is_empty() {
        report_nothing_to_open(&app, &group);
        return;
    }
    report_open_failure(&app, open_private_from(&app, &window, &group.links, group.browser));
}

/// 그룹을 열 때 이 페이지도 열지 (페이지 줄의 스위치)
#[tauri::command]
pub fn set_link_opens_with_group(app: AppHandle, group_id: Id, link_id: Id, opens: bool) {
    edit_store(
        &app,
        |store| if opens { tr!("{} 스위치 켜기", "Turn on {}", history::links(store, group_id, &[link_id])) } else { tr!("{} 스위치 끄기", "Turn off {}", history::links(store, group_id, &[link_id])) },
        |store| store.set_link_opens_with_group(group_id, link_id, opens),
    );
}

/// 그룹 안의 페이지들을 그 그룹의 브라우저로 연다. private 이거나 '항상 시크릿 모드'인 그룹이면 시크릿 창 하나에,
/// new_window 면 새 창 하나에 연다.
#[tauri::command(async)]
pub fn open_links(app: AppHandle, window: WebviewWindow, group_id: Id, ids: Vec<Id>, private: bool, new_window: Option<bool>) {
    let Some(group) = read_store(&app, |store| store.group(group_id).cloned()) else {
        return;
    };
    let ids = id_set(ids);
    let links: Vec<SavedLink> = group.links.iter().filter(|link| ids.contains(&link.id)).cloned().collect();
    if links.is_empty() {
        return;
    }
    let result = if private || group.always_private {
        open_private_from(&app, &window, &links, group.browser)
    } else if new_window.unwrap_or(false) {
        ChromeLauncher::open_links_in_new_window(&links, group.browser)
    } else {
        links.iter().try_for_each(|link| ChromeLauncher::open_link(link, group.browser))
    };
    report_open_failure(&app, result);
}

// MARK: - 아이콘

#[tauri::command(async)]
pub fn browser_icon(state: State<'_, AppState>, browser: Browser, size: u32) -> Option<String> {
    let size = size.clamp(16, 128);
    if let Some(known) = lock(&state.browser_icons).get(&(browser, size)) {
        return known.clone();
    }
    // 화면 배율 2배까지 선명하도록 2배 크기로 만든다.
    let icon = browser.icon_png(size * 2).map(|png| data_url(&png).to_string());
    lock(&state.browser_icons).insert((browser, size), icon.clone());
    icon
}

/// 페이지 주소의 사이트 아이콘 (data URL). 받지 못하면 None.
#[tauri::command]
pub async fn favicon(state: State<'_, AppState>, url: String) -> CommandResult<Option<String>> {
    Ok(state.favicons.icon(&url).await.map(|icon| icon.to_string()))
}

/// 그룹 아이콘 이미지 (data URL). 파일이 없으면 None.
#[tauri::command(async)]
pub fn group_icon_image(name: String) -> Option<String> {
    let path = GroupIconStore::file_path(&name, &LinkStore::data_directory())?;
    std::fs::read(path).ok().map(|png| data_url(&png).to_string())
}

/// 이미지 파일을 골라 그룹 아이콘으로 들여온다. 고르지 않았으면 None.
#[tauri::command]
pub async fn choose_group_icon_image(window: WebviewWindow) -> CommandResult<Option<String>> {
    let picked = blocking(move || {
        window
            .dialog()
            .file()
            .set_title(tr!("그룹 아이콘으로 쓸 이미지를 고르세요", "Choose an image for the group icon"))
            .add_filter(tr!("이미지", "Images"), &["png", "jpg", "jpeg", "gif", "webp", "bmp", "ico"])
            .set_parent(&window)
            .blocking_pick_file()
    })
    .await?;
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    GroupIconStore::import_image(&path, &LinkStore::data_directory()).map(Some).map_err(|error| error.to_string())
}

// MARK: - 브라우저 · 탭 가져오기

#[derive(Serialize)]
pub struct BrowserState {
    id: Browser,
    installed: bool,
    running: bool,
}

#[tauri::command(async)]
pub fn browser_states() -> Vec<BrowserState> {
    Browser::ALL
        .into_iter()
        .map(|browser| BrowserState { id: browser, installed: browser.is_installed(), running: browser.is_running() })
        .collect()
}

#[derive(Serialize)]
pub struct LoadedTabs {
    candidates: Vec<TabImportCandidate>,
    note: Option<String>,
}

/// 브라우저에 열린 탭을 읽어, 그 그룹에 넣을 수 있는 후보로 돌려준다.
#[tauri::command]
pub async fn load_tabs(app: AppHandle, browser: Browser, group_id: Id) -> CommandResult<LoadedTabs> {
    let existing = read_store(&app, |store| store.group(group_id).map(|group| group.links.clone())).unwrap_or_default();
    let host = AppTabsHost::new(&app);
    let result = blocking(move || ChromeTabSource::load(browser, &host)).await?.map_err(|error| error.to_string())?;
    Ok(LoadedTabs { candidates: TabImportCandidate::make(&result.tabs, &existing), note: result.note })
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ExistingGroupInfo {
    id: Id,
    name: String,
    color: GroupColor,
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LoadedTabGroup {
    tab_group: ChromeTabGroupImport,
    display_title: String,
    /// 같은 이름의 기존 그룹에 이미 있는 페이지는 '이미 있음'
    candidates: Vec<TabImportCandidate>,
    /// 같은 이름의 기존 그룹 (있으면 그 그룹에 더한다)
    existing: Option<ExistingGroupInfo>,
}

fn loaded_tab_groups(app: &AppHandle, tab_groups: Vec<ChromeTabGroupImport>) -> Vec<LoadedTabGroup> {
    read_store(app, |store| {
        tab_groups
            .into_iter()
            .map(|tab_group| {
                let existing = tab_group.matching_group(store.groups());
                LoadedTabGroup {
                    display_title: tab_group.info.display_title().to_owned(),
                    candidates: tab_group.candidates(existing),
                    existing: existing.map(|group| ExistingGroupInfo { id: group.id, name: group.name.clone(), color: group.color }),
                    tab_group,
                }
            })
            .collect()
    })
}

/// 브라우저에 저장된(닫아 둔) 탭 그룹. open 은 지금 열린 탭 그룹이다 (같은 이름 · 색은 열린 것으로 보고 뺀다).
/// 브라우저가 꺼져 있으면 open 이 비어 있어 저장된 그룹이 모두 닫힌 그룹으로 나온다.
#[tauri::command]
pub async fn load_closed_tab_groups(app: AppHandle, browser: Browser, open: Vec<ChromeTabGroupImport>) -> CommandResult<Vec<LoadedTabGroup>> {
    let closed = blocking(move || {
        let saved = linkgroupz_core::saved_tab_groups::read_saved_tab_groups(browser);
        linkgroupz_core::saved_tab_groups::closed_tab_groups(&saved, &open)
    })
    .await?;
    Ok(loaded_tab_groups(&app, closed))
}

/// 브라우저의 지금 탭 그룹. 확장이 있으면 확장 쪽(자동 저장 파일 → 세션 파일 → 마커 탭), 없으면 브라우저 세션 파일에서 읽는다
/// (crates/core/src/session_file.rs). 기다리는 일이 있어 백그라운드에서 부른다.
fn read_tab_groups(browser: Browser, host: &AppTabsHost) -> CommandResult<Vec<ChromeTabGroupImport>> {
    if ChromeExtensionDetector::find(browser).is_none() {
        if !browser.is_running() {
            return Err(linkgroupz_core::TabsError::BrowserNotRunning(browser).to_string());
        }
        if let Some(snapshot) = linkgroupz_core::session_file::SessionFileReader::read(browser) {
            return Ok(snapshot.tab_groups());
        }
        let name = browser.display_name();
        return Err(tr!("{name} 탭 그룹은 {name} 에 LinkGroupz 확장이 있어야 읽을 수 있습니다. 설정의 ‘확장 설치’를 참고해 주세요.", "Reading {name} tab groups needs the LinkGroupz extension in {name}. See “Install the Extension” in Settings.", name = name));
    }
    ChromeTabSource::snapshot(browser, host).map(|snapshot| snapshot.tab_groups()).map_err(|error| error.to_string())
}

/// 브라우저의 탭 그룹을 읽는다. 확장이 없으면 브라우저 세션 파일에서 읽는다.
#[tauri::command]
pub async fn load_tab_groups(app: AppHandle, browser: Browser) -> CommandResult<Vec<LoadedTabGroup>> {
    let host = AppTabsHost::new(&app);
    let tab_groups = blocking(move || read_tab_groups(browser, &host)).await??;
    Ok(loaded_tab_groups(&app, tab_groups))
}

// MARK: - 그룹 동기화 (crates/core/src/group_sync.rs)

/// 그룹과 브라우저에 같은 이름으로 열린 탭 그룹을 비교한 결과
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncPlan {
    browser: Browser,
    tab_group_title: String,
    /// 탭 그룹의 페이지 (순서대로)
    tabs: Vec<linkgroupz_core::group_sync::SyncTab>,
    #[serde(flatten)]
    comparison: linkgroupz_core::group_sync::SyncComparison,
}

/// 그룹을 브라우저(그룹에 정한 브라우저)의 같은 이름 탭 그룹과 비교한다.
#[tauri::command]
pub async fn sync_group_plan(app: AppHandle, id: Id) -> CommandResult<SyncPlan> {
    let group = read_store(&app, |store| store.group(id).cloned()).ok_or(tr!("그룹을 찾지 못했습니다.", "Couldn't find the group."))?;
    let host = AppTabsHost::new(&app);
    let browser = group.browser;
    let tab_groups = blocking(move || read_tab_groups(browser, &host)).await??;
    let Some(tab_group) = linkgroupz_core::group_sync::find_tab_group(&tab_groups, &group.name) else {
        return Err(tr!("{} 에 ‘{}’ 이름의 탭 그룹이 열려 있지 않습니다. 같은 이름의 탭 그룹을 연 뒤 다시 해 주세요.", "{} has no open tab group named “{}”. Open a tab group with the same name and try again.", browser.display_name(), group.name));
    };
    let tabs = linkgroupz_core::group_sync::tabs_of(tab_group);
    Ok(SyncPlan {
        browser,
        tab_group_title: tab_group.info.title.clone(),
        comparison: linkgroupz_core::group_sync::compare(&group.links, &tabs),
        tabs,
    })
}

/// 동기화 창에서 고른 것
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncChoice {
    tabs: Vec<linkgroupz_core::group_sync::SyncTab>,
    /// 탭 그룹 전체로 덮어쓰기
    replace_all: bool,
    /// 더할 탭 (tabs 의 번호)
    add: Vec<usize>,
    /// 뺄 페이지
    remove: Vec<Id>,
    use_title_as_alias: bool,
}

#[tauri::command]
pub fn sync_group_apply(app: AppHandle, id: Id, choice: SyncChoice) -> CommandResult {
    edit_store(&app, |store| tr!("{} 동기화", "Sync {}", history::groups(store, &[id])), |store| {
        let group = store.group(id).ok_or(tr!("그룹을 찾지 못했습니다.", "Couldn't find the group."))?;
        let links = if choice.replace_all {
            linkgroupz_core::group_sync::replaced(&group.links, &choice.tabs, choice.use_title_as_alias)
        } else {
            linkgroupz_core::group_sync::merged(&group.links, &choice.tabs, &choice.add, &id_set(choice.remove.clone()), choice.use_title_as_alias)
        };
        store.set_group_links(id, links);
        Ok(())
    })
}

/// 탭 그룹 가져오기 화면이 보내는 선택
#[derive(Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroupSelection {
    tab_groups: Vec<ChromeTabGroupImport>,
    /// 탭 그룹 id → 고른 후보 id
    selections: HashMap<i64, Vec<usize>>,
    use_title_as_alias: bool,
    browser: Browser,
}

impl TabGroupSelection {
    fn plan(&self, existing_groups: &[LinkGroup]) -> TabGroupImportPlan {
        let selections: HashMap<i64, HashSet<usize>> =
            self.selections.iter().map(|(id, chosen)| (*id, chosen.iter().copied().collect())).collect();
        TabGroupImportPlan::make(&self.tab_groups, &selections, existing_groups, self.use_title_as_alias, self.browser)
    }
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PlanSummary {
    new_groups: usize,
    additions: usize,
    page_count: usize,
}

/// 고른 대로 가져오면 무엇이 생기는지 (가져오기 버튼에 적는다)
#[tauri::command]
pub fn plan_tab_group_import(app: AppHandle, selection: TabGroupSelection) -> PlanSummary {
    let plan = read_store(&app, |store| selection.plan(store.groups()));
    PlanSummary { new_groups: plan.new_groups.len(), additions: plan.additions.len(), page_count: plan.page_count() }
}

/// 고른 탭 그룹 · 페이지를 가져온다. 화면이 바로 보여 줄 그룹(처음 만들었거나 더한 그룹)을 돌려준다.
#[tauri::command]
pub fn import_tab_groups(app: AppHandle, selection: TabGroupSelection) -> Option<Id> {
    edit_store(&app, |_| tr!("탭 그룹 가져오기", "Import tab groups").to_owned(), |store| {
        let plan = selection.plan(store.groups());
        let first = plan.new_groups.first().map(|group| group.id).or_else(|| plan.additions.first().map(|addition| addition.group_id));
        for group in plan.new_groups {
            store.add_group(group);
        }
        for addition in plan.additions {
            store.add_links(addition.links, addition.group_id);
        }
        first
    })
}

// MARK: - 설정 · 확장

// MARK: - 내보내기 · 가져오기

/// 지금 데이터(그룹 · 페이지 · 섹션 · 보관함, 그룹 아이콘 이미지)를 고른 파일로 내보낸다. 저장한 파일 경로를 돌려준다 (취소하면 None).
#[tauri::command]
pub async fn export_data(app: AppHandle, window: WebviewWindow) -> CommandResult<Option<String>> {
    let data = linkgroupz_core::transfer::with_icon_images(&read_store(&app, LinkStore::export_data), &LinkStore::data_directory());
    let profile = crate::profiles::current(&app);
    let profile_part = if profile.is_default() { String::new() } else { format!("-{}", profile.name.replace(['/', '\\', ':'], "_")) };
    let file_name = format!("LinkGroupz{profile_part}-{}.json", linkgroupz_core::backups::Backups::today().format("%Y-%m-%d"));
    let picked = blocking(move || {
        window
            .dialog()
            .file()
            .set_title(tr!("데이터 내보내기", "Export Data"))
            .set_file_name(&file_name)
            .add_filter(tr!("LinkGroupz 데이터", "LinkGroupz data"), &["json"])
            .set_parent(&window)
            .blocking_save_file()
    })
    .await?;
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    linkgroupz_core::write_atomic(&path, &data).map_err(|error| tr!("저장하지 못했습니다: {error}", "Couldn't save: {error}", error = error))?;
    Ok(Some(display_path(&path)))
}

/// 가져올 파일 (고른 뒤 무엇이 들었는지 보여 주고, 가져오는 방법을 묻는다)
#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    path: String,
    file_name: String,
    summary: linkgroupz_core::transfer::ImportSummary,
}

/// 가져올 데이터 파일을 고르고 읽어 본다 (아직 가져오지 않는다). 취소하면 None.
#[tauri::command]
pub async fn choose_import_file(window: WebviewWindow) -> CommandResult<Option<ImportCandidate>> {
    let picked = blocking(move || {
        window
            .dialog()
            .file()
            .set_title(tr!("가져올 데이터 파일을 고르세요 (LinkGroupz · LinkGroups 의 data.json 또는 내보낸 파일)", "Choose a data file to import (LinkGroupz or LinkGroups data.json, or an exported file)"))
            .add_filter(tr!("LinkGroupz 데이터", "LinkGroupz data"), &["json"])
            .set_parent(&window)
            .blocking_pick_file()
    })
    .await?;
    let Some(path) = picked.and_then(|file| file.into_path().ok()) else {
        return Ok(None);
    };
    let data = std::fs::read(&path).map_err(|error| tr!("파일을 읽지 못했습니다: {error}", "Couldn't read the file: {error}", error = error))?;
    let document = linkgroupz_core::transfer::read_document(&data).map_err(|error| error.to_string())?;
    Ok(Some(ImportCandidate {
        file_name: path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default(),
        path: path.to_string_lossy().into_owned(),
        summary: linkgroupz_core::transfer::ImportSummary::of_file(&document, &data),
    }))
}

/// 고른 파일을 가져온다. 바꾸기(replace)면 지금 데이터를 백업 폴더에 따로 남긴 뒤 바꾼다.
#[tauri::command]
pub async fn import_data(app: AppHandle, path: String, mode: linkgroupz_core::transfer::ImportMode) -> CommandResult {
    let data = std::fs::read(&path).map_err(|error| tr!("파일을 읽지 못했습니다: {error}", "Couldn't read the file: {error}", error = error))?;
    let document = linkgroupz_core::transfer::read_document(&data).map_err(|error| error.to_string())?;
    if mode == linkgroupz_core::transfer::ImportMode::Replace {
        let current = linkgroupz_core::transfer::with_icon_images(&read_store(&app, LinkStore::export_data), &LinkStore::data_directory());
        let folder = backup_directory(&app);
        std::fs::create_dir_all(&folder).map_err(|error| error.to_string())?;
        let keep = folder.join(format!("data-before-import-{}.json", linkgroupz_core::backups::Backups::now_stamp()));
        linkgroupz_core::write_atomic(&keep, &current).map_err(|error| tr!("지금 데이터를 남기지 못해 가져오지 않았습니다: {error}", "Didn't import because the current data couldn't be kept: {error}", error = error))?;
    }
    // 함께 든 아이콘 이미지를 먼저 되살린다 (그룹이 쓰는 이미지라 정리할 때 지워지지 않는다).
    linkgroupz_core::transfer::restore_icon_images(&data, &LinkStore::data_directory());
    edit_store(&app, |_| tr!("데이터 가져오기", "Import data").to_owned(), |store| store.import_document(document, mode));
    remove_unused_icons(&app);
    Ok(())
}

// MARK: - 백업

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupState {
    /// 며칠치 남기는지 (0 이면 백업하지 않음)
    days: u32,
    choices: Vec<u32>,
    /// '무제한'(지우지 않음)을 뜻하는 값
    keep_all: u32,
    backups: Vec<linkgroupz_core::backups::BackupInfo>,
    folder: String,
}

/// 지금 프로필의 백업 폴더
fn backup_directory(app: &AppHandle) -> PathBuf {
    crate::profiles::current(app).backup_directory(&LinkStore::data_directory())
}

/// 지금 프로필의 백업 목록
#[tauri::command(async)]
pub fn backups(app: AppHandle) -> BackupState {
    let days = lock(&app.state::<AppState>().settings).backup_days;
    BackupState {
        days,
        choices: linkgroupz_core::backups::BACKUP_DAY_CHOICES.to_vec(),
        keep_all: linkgroupz_core::backups::KEEP_ALL_BACKUPS,
        backups: linkgroupz_core::backups::Backups::list(&backup_directory(&app)),
        folder: display_path(&backup_directory(&app)),
    }
}

/// 보관 기간을 바꾼다. 줄이면 기간이 지난 백업은 다음 확인 때(1시간 안) 지워진다.
/// 화면은 바꾸기 전에 지워질 백업 수를 보여 주고 묻는다 (`expired_count`).
#[tauri::command(async)]
pub fn set_backup_days(app: AppHandle, days: u32) -> BackupState {
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        settings.backup_days = days;
        settings.save();
    }
    // 바로 맞춘다: 켰으면 오늘 백업을 만들고, 줄였으면 지난 것을 지운다 (모든 프로필).
    crate::run_backups_now(&app);
    backups(app)
}

/// 백업한 날의 그룹 데이터로 되돌린다. 지금 데이터는 백업 폴더에 따로 남긴다.
#[tauri::command(async)]
pub fn restore_backup(app: AppHandle, date: String) -> CommandResult {
    let folder = backup_directory(&app);
    let backup = linkgroupz_core::backups::Backups::list(&folder)
        .into_iter()
        .find(|info| info.date == date)
        .ok_or(tr!("그 날짜의 백업을 찾지 못했습니다.", "Couldn't find the backup for that date."))?;
    let state = app.state::<AppState>();
    let snapshot = {
        let mut store = lock(&state.store);
        linkgroupz_core::backups::Backups::restore(
            &backup.path,
            store.file_path(),
            &folder,
            &linkgroupz_core::backups::Backups::now_stamp(),
            &LinkStore::data_directory(),
        )
        .map_err(|error| error.to_string())?;
        let before = store.document();
        store.load();
        state.history.record(store.file_path(), tr!("{date} 백업으로 되돌리기", "Restore the {date} backup", date = date), before, store.document());
        StoreSnapshot::of(&store)
    };
    let _ = tauri::Emitter::emit(&app, crate::state::STORE_CHANGED, snapshot);
    Ok(())
}

/// 앱을 직접 켤 때 관리 창을 열지
#[tauri::command]
pub fn set_show_manager_on_launch(state: State<'_, AppState>, show: bool) {
    let mut settings = lock(&state.settings);
    settings.show_manager_on_launch = show;
    settings.save();
}

/// Dock 에 아이콘을 보일지 (macOS). 끄면 메뉴바 아이콘으로만 쓴다.
#[tauri::command]
pub fn set_show_dock_icon(app: AppHandle, show: bool) {
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        settings.show_dock_icon = show;
        settings.save();
    }
    crate::apply_dock_icon(&app);
}

#[tauri::command]
pub fn set_group_open_mode(state: State<'_, AppState>, mode: GroupOpenMode) {
    let mut settings = lock(&state.settings);
    settings.group_open_mode = mode;
    settings.save();
}

#[derive(Serialize)]
pub struct ExtensionState {
    browser: Browser,
    installed: bool,
    found: Option<FoundExtension>,
}

/// 브라우저마다 확장이 감지되는지. 확장을 방금 설치했으면 탭 목록 자동 저장 연결도 바로 등록한다.
#[tauri::command(async)]
pub fn extension_states() -> Vec<ExtensionState> {
    let states = Browser::ALL
        .into_iter()
        .map(|browser| ExtensionState {
            browser,
            installed: browser.is_installed(),
            found: ChromeExtensionDetector::find(browser),
        })
        .collect();
    NativeSnapshotStore::install_if_possible();
    states
}

/// 앱에 든 확장 폴더
pub fn bundled_extension_dir(app: &AppHandle) -> Option<PathBuf> {
    let folder = app.path().resource_dir().ok()?.join("chrome-extension");
    folder.join("manifest.json").is_file().then_some(folder)
}

/// 브라우저에 로드할 확장 폴더 (앱에 든 확장을 데이터 폴더로 복사해 둔 곳)
fn synced_extension_dir(app: &AppHandle) -> Option<PathBuf> {
    BundledExtension::sync(bundled_extension_dir(app).as_deref(), &BundledExtension::installed_dir())
}

#[tauri::command(async)]
pub fn extension_folder(app: AppHandle) -> Option<String> {
    synced_extension_dir(&app).map(|folder| display_path(&folder))
}

#[tauri::command(async)]
pub fn reveal_extension_folder(app: AppHandle) {
    if let Some(folder) = synced_extension_dir(&app) {
        let _ = app.opener().reveal_item_in_dir(folder.join("manifest.json"));
    }
}

#[tauri::command(async)]
pub fn reveal_data_file(app: AppHandle) {
    let file = read_store(&app, |store| store.file_path().to_owned());
    let target = if file.exists() { file } else { LinkStore::data_directory() };
    let _ = app.opener().reveal_item_in_dir(target);
}

#[tauri::command]
pub fn copy_text(app: AppHandle, text: String) -> CommandResult {
    app.clipboard().write_text(text).map_err(|error| error.to_string())
}

#[derive(Serialize)]
#[serde(rename_all = "camelCase")]
pub struct LaunchAtLogin {
    state: LaunchAtLoginState,
    /// DMG 안처럼 옮겨질 수 있는 곳에서 실행 중이면 켤 수 없다
    installed_location: bool,
}

#[tauri::command(async)]
pub fn launch_at_login(app: AppHandle) -> LaunchAtLogin {
    LaunchAtLogin { state: launch::state(&app), installed_location: launch::is_installed_location() }
}

#[tauri::command(async)]
pub fn set_launch_at_login(app: AppHandle, enabled: bool) -> CommandResult<LaunchAtLogin> {
    launch::set(&app, enabled).map_err(|error| tr!("설정하지 못했습니다: {error}", "Couldn't change the setting: {error}", error = error))?;
    Ok(launch_at_login(app))
}

#[tauri::command(async)]
pub fn open_login_items_settings() {
    launch::open_login_items_settings();
}

// MARK: - 미리보기

/// 페이지 줄의 눈 버튼: 그 페이지의 작은 미리보기를 띄우거나, 이미 떠 있으면 닫는다. 띄웠으면 true.
#[tauri::command]
pub async fn toggle_preview(app: AppHandle, group_id: Id, link_id: Id, anchor: Option<Anchor>) -> CommandResult<bool> {
    if app.state::<AppState>().previews.compact_link() == Some(link_id) {
        preview::close(&app, preview::COMPACT);
        return Ok(false);
    }
    let Some((group, link)) = read_store(&app, |store| {
        let group = store.group(group_id)?;
        let link = group.links.iter().find(|link| link.id == link_id)?;
        Some((group.clone(), link.clone()))
    }) else {
        return Ok(false);
    };
    let request = PreviewRequest {
        url: link.url.as_str().to_owned(),
        fallback_title: link.display_name(),
        browser: group.browser,
        is_private: group.always_private,
        link_id: Some(link.id),
        compact: true,
        anchor,
    };
    blocking(move || preview::open(&app, request)).await?
}

#[tauri::command(async)]
pub fn close_preview(app: AppHandle) {
    preview::close(&app, preview::COMPACT);
}

/// 작은 미리보기가 보여 주는 페이지 (관리 창이 눈 버튼을 채워 그릴 때 쓴다)
#[tauri::command]
pub fn previewing_link(state: State<'_, AppState>) -> Option<Id> {
    state.previews.compact_link()
}

/// 미리보기 창의 위쪽 띠가 자기 창의 내용을 묻는다.
fn preview_of(webview: &Webview) -> Option<PreviewInfo> {
    let label = preview::window_label_of_bar(webview.label())?;
    webview.app_handle().state::<AppState>().previews.info(label)
}

#[tauri::command]
pub fn preview_info(webview: Webview) -> Option<PreviewInfo> {
    preview_of(&webview)
}

#[tauri::command(async)]
pub fn preview_back(webview: Webview) {
    if let Some(info) = preview_of(&webview) {
        preview::go_back(webview.app_handle(), &info.label);
    }
}

#[tauri::command(async)]
pub fn preview_forward(webview: Webview) {
    if let Some(info) = preview_of(&webview) {
        preview::go_forward(webview.app_handle(), &info.label);
    }
}

#[tauri::command(async)]
pub fn preview_reload(webview: Webview) {
    if let Some(info) = preview_of(&webview) {
        preview::reload(webview.app_handle(), &info.label);
    }
}

/// 작은 미리보기의 '크게 열기': 지금 보고 있는 주소를 원래 크기 창으로 연다 (로그인이 필요하면 여기서 한다).
#[tauri::command]
pub async fn preview_expand(webview: Webview) -> CommandResult {
    let Some(info) = preview_of(&webview) else {
        return Ok(());
    };
    let app = webview.app_handle().clone();
    let request = PreviewRequest {
        url: info.url,
        fallback_title: info.fallback_title,
        browser: info.browser,
        is_private: info.is_private,
        link_id: None,
        compact: false,
        anchor: None,
    };
    blocking(move || preview::open(&app, request)).await?.map(|_| ())
}

/// 미리보기에서 보고 있는 주소를 브라우저로 연다. browser 를 주지 않으면 그 페이지가 속한 그룹의 브라우저.
#[tauri::command(async)]
pub fn preview_open_in_browser(webview: Webview, browser: Option<Browser>) {
    let Some(info) = preview_of(&webview) else {
        return;
    };
    let link = SavedLink::new(LinkUrl::new(info.url), None);
    let browser = browser.unwrap_or(info.browser);
    let result = if info.is_private {
        ChromeLauncher::open_private(std::slice::from_ref(&link), browser)
    } else {
        ChromeLauncher::open_link(&link, browser)
    };
    report_open_failure(webview.app_handle(), result);
}

/// 보관해 둔 미리보기 로그인의 사이트 수. 셀 수 없는 운영체제면 None.
/// 키체인을 읽으므로 macOS 가 키체인 접근을 물을 수 있다.
#[tauri::command(async)]
pub fn preview_site_count(app: AppHandle) -> Option<usize> {
    preview::stored_site_count(&app)
}

#[tauri::command(async)]
pub fn clear_preview_logins(app: AppHandle) {
    preview::clear_logins(&app);
}

// MARK: - 업데이트

#[tauri::command]
pub fn update_status(state: State<'_, AppState>) -> UpdateStatus {
    state.updater.status()
}

#[tauri::command(async)]
pub fn update_set_auto_check(app: AppHandle, enabled: bool) {
    let updater = app.state::<AppState>().updater.clone();
    updater.set_auto_check(&app, enabled);
}

#[tauri::command]
pub async fn update_check(app: AppHandle) -> CommandResult {
    let updater = app.state::<AppState>().updater.clone();
    updater.check(&app, true).await;
    Ok(())
}

#[tauri::command]
pub async fn update_install(app: AppHandle) -> CommandResult {
    let updater = app.state::<AppState>().updater.clone();
    updater.install(&app).await;
    Ok(())
}

/// 업데이트 창의 '나중에': 이 버전은 다시 묻지 않고 창을 닫는다.
#[tauri::command]
pub fn update_later(app: AppHandle, window: WebviewWindow) {
    app.state::<AppState>().updater.later(&app);
    let _ = window.close();
}

#[tauri::command]
pub fn update_clear_message(app: AppHandle) {
    app.state::<AppState>().updater.clear_message(&app);
}

/// 배포 파일이 올라오는 GitHub 릴리스 페이지를 기본 브라우저로 연다.
#[tauri::command(async)]
pub fn open_update_page(app: AppHandle) {
    if let Some(source) = app.state::<AppState>().updater.source() {
        let _ = app.opener().open_url(source.releases_page_url(), None::<&str>);
    }
}

// MARK: - 창

#[tauri::command(async)]
pub fn show_manager(app: AppHandle) {
    windows::hide_popover(&app);
    windows::show_manager(&app);
}

#[tauri::command(async)]
pub fn show_settings(app: AppHandle) {
    windows::hide_popover(&app);
    windows::show_settings(&app);
}

/// 메뉴바 창의 오른쪽 클릭 메뉴에서 고른 일을 관리 창을 열어 이어서 한다 (편집 · 페이지 추가 · 삭제 확인 등).
/// 관리 창이 아직 없으면 뜬 뒤에 `take_manager_action` 으로 가져간다.
#[tauri::command(async)]
pub fn request_manager_action(app: AppHandle, action: serde_json::Value) {
    *lock(&app.state::<AppState>().manager_action) = Some(action);
    windows::hide_popover(&app);
    windows::show_manager(&app);
    let _ = tauri::Emitter::emit(&app, "manager-action", ());
}

#[tauri::command]
pub fn take_manager_action(state: State<'_, AppState>) -> Option<serde_json::Value> {
    lock(&state.manager_action).take()
}

#[tauri::command(async)]
pub fn show_welcome(app: AppHandle) {
    windows::hide_popover(&app);
    windows::show_welcome(&app);
}

/// '바뀐 점' 창에 보여 줄 내용
#[derive(Clone, Serialize)]
pub struct WhatsNew {
    pub title: String,
    pub entries: Vec<ReleaseEntry>,
}

/// 설정에서 다시 볼 때: 최근 버전들의 바뀐 점
#[tauri::command(async)]
pub fn show_whats_new(app: AppHandle) {
    let state = app.state::<AppState>();
    let current = state.updater.current_version().clone();
    let entries: Vec<ReleaseEntry> =
        ReleaseNotes::entries().iter().filter(|entry| entry.version <= current).take(6).map(linkgroupz_core::ReleaseEntry::localized).collect();
    *lock(&state.whats_new) = Some(WhatsNew { title: tr!("LinkGroupz 바뀐 점", "What's New in LinkGroupz").to_owned(), entries });
    windows::show_whats_new(&app);
}

#[tauri::command]
pub fn whats_new_content(state: State<'_, AppState>) -> Option<WhatsNew> {
    lock(&state.whats_new).clone()
}

/// 앱에 든 사용법(HTML)
/// 영어 화면이면 영어판(guide/en)을, 없으면 한국어판을 연다.
fn guide_file(app: &AppHandle) -> Option<PathBuf> {
    let guide = app.path().resource_dir().ok()?.join("guide");
    let english = guide.join("en").join("index.html");
    if linkgroupz_core::i18n::is_english() && english.is_file() {
        return Some(english);
    }
    let file = guide.join("index.html");
    file.is_file().then_some(file)
}

#[tauri::command(async)]
pub fn open_guide(app: AppHandle) {
    if let Some(file) = guide_file(&app) {
        let _ = app.opener().open_path(file.to_string_lossy(), None::<&str>);
    }
}

/// 앱에 든 외부 코드의 저작권 · 라이선스 고지(HTML). `cargo xtask licenses` 가 만든다.
fn licenses_file(app: &AppHandle) -> Option<PathBuf> {
    let file = app.path().resource_dir().ok()?.join("THIRD-PARTY-LICENSES.html");
    file.is_file().then_some(file)
}

#[tauri::command(async)]
pub fn open_licenses(app: AppHandle) {
    if let Some(file) = licenses_file(&app) {
        let _ = app.opener().open_path(file.to_string_lossy(), None::<&str>);
    }
}

// MARK: - 단축키

#[tauri::command(async)]
pub fn popover_shortcut(app: AppHandle) -> ShortcutStatus {
    shortcut::status(&app)
}

/// 메뉴바 창을 여닫는 단축키를 바꾼다 (None 이면 없앤다).
#[tauri::command(async)]
pub fn set_popover_shortcut(app: AppHandle, shortcut: Option<String>) -> Result<ShortcutStatus, String> {
    shortcut::set(&app, shortcut).map_err(|error| error.to_string())
}

/// 설정 창에서 새 단축키를 받는 동안 단축키 듣기를 멈춘다.
#[tauri::command(async)]
pub fn pause_popover_shortcut(app: AppHandle, paused: bool) {
    shortcut::pause(&app, paused);
}

#[tauri::command]
pub fn quit(app: AppHandle) {
    app.exit(0);
}

#[tauri::command(async)]
pub fn close_window(window: WebviewWindow) {
    let _ = window.close();
}

/// 내용 높이에 맞춰 창 높이를 바꾼다 (시작 안내 · 바뀐 점).
/// content 는 내용의 높이, viewport 는 지금 화면에 보이는 높이.
#[tauri::command(async)]
pub fn fit_window_height(window: WebviewWindow, content: f64, viewport: f64) {
    windows::fit_height(&window, content, viewport);
}

/// 메뉴바 창의 내용 높이가 바뀌었다.
#[tauri::command(async)]
pub fn resize_popover(app: AppHandle, height: f64) {
    windows::resize_popover(&app, height);
}

/// 사용자가 메뉴바 창의 손잡이를 끌어 크기를 정한다 (size 가 없으면 다시 내용에 맞춘다).
#[tauri::command(async)]
pub fn set_popover_size(app: AppHandle, size: Option<(f64, f64)>, done: bool) {
    windows::set_popover_size(&app, size, done);
}

/// 메뉴바 창을 메뉴바에서 떼어 놓거나 다시 붙인다.
#[tauri::command(async)]
pub fn set_popover_detached(app: AppHandle, detached: bool) {
    windows::set_popover_detached(&app, detached);
}

#[tauri::command(async)]
pub fn popover_layout(app: AppHandle) -> windows::PopoverLayout {
    windows::popover_layout(&app)
}

#[tauri::command(async)]
pub fn hide_popover(app: AppHandle) {
    windows::hide_popover(&app);
}

// MARK: - 되돌리기 (crates/app/src/history.rs)

/// 마지막으로 바꾼 일을 되돌리고, 무엇을 되돌렸는지 돌려준다. 되돌릴 것이 없으면 None.
#[tauri::command]
pub fn undo(app: AppHandle) -> Option<String> {
    history::step(&app, history::Step::Undo)
}

/// 되돌린 일을 다시 하고, 무엇을 다시 했는지 돌려준다. 다시 할 것이 없으면 None.
#[tauri::command]
pub fn redo(app: AppHandle) -> Option<String> {
    history::step(&app, history::Step::Redo)
}

/// 글자 칸의 실행 취소 · 복귀 (macOS 편집 메뉴가 원래 하던 일, history::edit_menu). Windows 는 화면이 그대로 처리해 부르지 않는다.
#[tauri::command]
pub fn native_edit(app: AppHandle, redo: bool) {
    #[cfg(target_os = "macos")]
    {
        let _ = app.run_on_main_thread(move || history::edit_menu::send_native(redo));
    }
    #[cfg(not(target_os = "macos"))]
    {
        let _ = (app, redo);
    }
}
