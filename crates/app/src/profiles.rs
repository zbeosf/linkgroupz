//! 프로필 전환 · 추가 · 이름 바꾸기 · 삭제 (저장 구조는 crates/core/src/profiles.rs).
//!
//! 프로필을 바꾸면 저장소를 그 프로필의 data.json 으로 다시 열고, 모든 창에 새 그룹 목록(store-changed)과
//! 프로필 목록(profiles-changed)을 알린다. 프로필 추가 · 이름 바꾸기 · 삭제는 설정 창에서만 한다.

use linkgroupz_core::tr;
use crate::state::{lock, AppState, StoreSnapshot, STORE_CHANGED};
use linkgroupz_core::profiles::{self, Profile};
use linkgroupz_core::{GroupColor, GroupIcon, GroupIconStore, LinkGroup, LinkStore};
use serde::Serialize;
use tauri::{AppHandle, Emitter, Manager};

pub const PROFILES_CHANGED: &str = "profiles-changed";

type CommandResult<T = ()> = Result<T, String>;

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileItem {
    id: String,
    name: String,
    icon: GroupIcon,
    color: GroupColor,
    is_default: bool,
    /// 그룹 수 · 페이지 수 (보관함 제외). 파일을 읽지 못하면 None
    groups: Option<usize>,
    links: Option<usize>,
}

#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ProfileState {
    profiles: Vec<ProfileItem>,
    /// 지금 프로필 id
    current: String,
}

/// 지금 프로필
pub fn current(app: &AppHandle) -> Profile {
    lock(&app.state::<AppState>().settings).current_profile()
}

/// 모든 프로필 (맨 앞은 기본)
pub fn all(app: &AppHandle) -> Vec<Profile> {
    lock(&app.state::<AppState>().settings).profiles()
}

/// 프로필의 그룹 · 보관 그룹. 지금 프로필은 저장소에서, 다른 프로필은 파일에서 읽는다.
/// 파일이 없으면(아직 저장한 적 없는 프로필) 빈 목록, 읽을 수 없으면 None.
fn groups_of(app: &AppHandle, profile: &Profile, current_id: &str) -> Option<(Vec<LinkGroup>, Vec<LinkGroup>)> {
    if profile.id == current_id {
        let state = app.state::<AppState>();
        let store = lock(&state.store);
        return Some((store.groups().to_vec(), store.archive().groups.iter().map(|item| item.group.clone()).collect()));
    }
    let file = profile.data_file(&LinkStore::data_directory());
    if !file.exists() {
        return Some((Vec::new(), Vec::new()));
    }
    let document = linkgroupz_core::transfer::read_document(&std::fs::read(file).ok()?).ok()?;
    Some((document.groups, document.archive.groups.into_iter().map(|item| item.group).collect()))
}

/// 모든 프로필의 그룹(보관함 포함). 한 프로필이라도 읽지 못하면 None (그때는 그룹 아이콘 이미지를 정리하지 않는다).
pub fn groups_in_all_profiles(app: &AppHandle) -> Option<Vec<LinkGroup>> {
    let current_id = current(app).id;
    let mut all_groups = Vec::new();
    for profile in all(app) {
        let (groups, archived) = groups_of(app, &profile, &current_id)?;
        all_groups.extend(groups);
        all_groups.extend(archived);
    }
    Some(all_groups)
}

pub fn state(app: &AppHandle) -> ProfileState {
    let current_id = current(app).id;
    let profiles = all(app)
        .into_iter()
        .map(|profile| {
            let counts = groups_of(app, &profile, &current_id).map(|(groups, _)| (groups.len(), groups.iter().map(|group| group.links.len()).sum()));
            ProfileItem {
                is_default: profile.is_default(),
                groups: counts.map(|(groups, _)| groups),
                links: counts.map(|(_, links)| links),
                name: profile.display_name(),
                id: profile.id,
                icon: profile.icon,
                color: profile.color,
            }
        })
        .collect();
    ProfileState { profiles, current: current_id }
}

/// 프로필 목록이 바뀐 것을 모든 창과 메뉴바(트레이) 메뉴에 알린다.
fn notify(app: &AppHandle) {
    let _ = app.emit(PROFILES_CHANGED, state(app));
    crate::tray::refresh_menu(app);
}

/// 프로필을 바꾼다: 설정에 적고, 저장소를 그 프로필의 파일로 다시 연다.
pub fn switch_to(app: &AppHandle, id: &str) -> CommandResult {
    let state = app.state::<AppState>();
    let profile = {
        let mut settings = lock(&state.settings);
        let profile = settings.profiles().into_iter().find(|profile| profile.id == id).ok_or(tr!("그 프로필을 찾지 못했습니다.", "Couldn't find that profile."))?;
        if settings.current_profile == profile.id {
            return Ok(());
        }
        settings.current_profile = profile.id.clone();
        settings.save();
        profile
    };
    // 설정 잠금을 푼 뒤 저장소를 잠근다 (두 잠금을 겹쳐 잡지 않는다).
    let snapshot = {
        let mut store = lock(&state.store);
        *store = LinkStore::open(profile.data_file(&LinkStore::data_directory()));
        StoreSnapshot::of(&store)
    };
    let _ = app.emit(STORE_CHANGED, snapshot);
    notify(app);
    Ok(())
}

fn same_name(a: &str, b: &str) -> bool {
    a.to_lowercase() == b.to_lowercase()
}

#[tauri::command(async)]
pub fn profiles(app: AppHandle) -> ProfileState {
    state(&app)
}

#[tauri::command(async)]
pub fn switch_profile(app: AppHandle, id: String) -> CommandResult {
    switch_to(&app, &id)
}

/// 프로필을 더한다 (바꾸지는 않는다). 새 프로필 id 를 돌려준다.
#[tauri::command(async)]
pub fn add_profile(app: AppHandle, name: String) -> CommandResult<String> {
    let profile = Profile::new(&name).ok_or(tr!("이름을 1~{}글자로 적어 주세요.", "Enter a name of 1 to {} characters.", profiles::NAME_LIMIT))?;
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        let mut list = settings.profiles();
        if list.iter().any(|other| same_name(&other.name, &profile.name)) {
            return Err(tr!("같은 이름의 프로필이 이미 있습니다.", "A profile with this name already exists.").to_owned());
        }
        list.push(profile.clone());
        settings.profiles = list;
        settings.save();
    }
    notify(&app);
    Ok(profile.id)
}

/// 프로필 이름 · 아이콘 · 색을 바꾼다.
#[tauri::command(async)]
pub fn update_profile(app: AppHandle, id: String, name: String, icon: GroupIcon, color: GroupColor) -> CommandResult {
    let name = profiles::clean_name(&name).ok_or(tr!("이름을 1~{}글자로 적어 주세요.", "Enter a name of 1 to {} characters.", profiles::NAME_LIMIT))?;
    if let Some(file) = icon.image_file_name() {
        GroupIconStore::file_path(file, &LinkStore::data_directory()).filter(|path| path.is_file()).ok_or(tr!("아이콘 이미지를 찾지 못했습니다. 다시 골라 주세요.", "Couldn't find the icon image. Please choose it again."))?;
    }
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        let mut list = settings.profiles();
        if list.iter().any(|other| other.id != id && same_name(&other.name, &name)) {
            return Err(tr!("같은 이름의 프로필이 이미 있습니다.", "A profile with this name already exists.").to_owned());
        }
        let profile = list.iter_mut().find(|profile| profile.id == id).ok_or(tr!("그 프로필을 찾지 못했습니다.", "Couldn't find that profile."))?;
        profile.name = name;
        profile.icon = icon;
        profile.color = color;
        settings.profiles = list;
        settings.save();
    }
    notify(&app);
    crate::commands::remove_unused_icons(&app);
    Ok(())
}

/// 프로필을 지운다. 지금 프로필이면 기본 프로필로 바꾼 뒤 지운다.
/// 그 프로필의 폴더(data.json · 백업)는 지우지 않고 기본 백업 폴더의 deleted-profiles 로 옮긴다.
/// 옮긴 data.json 에는 그룹 아이콘 이미지도 넣어 둔다 (가져오기로 되살리면 아이콘까지 돌아온다).
#[tauri::command(async)]
pub fn delete_profile(app: AppHandle, id: String) -> CommandResult {
    let profile = all(&app).into_iter().find(|profile| profile.id == id).ok_or(tr!("그 프로필을 찾지 못했습니다.", "Couldn't find that profile."))?;
    if profile.is_default() {
        return Err(tr!("기본 프로필은 지울 수 없습니다.", "The default profile can't be deleted.").to_owned());
    }
    if current(&app).id == profile.id {
        switch_to(&app, profiles::DEFAULT_PROFILE_ID)?;
    }
    let data_directory = LinkStore::data_directory();
    let moved = profile
        .retire(&data_directory, &linkgroupz_core::backups::Backups::now_stamp())
        .map_err(|error| tr!("프로필 폴더를 옮기지 못해 지우지 않았습니다: {}", "Didn't delete because the profile folder couldn't be moved: {}", error))?;
    if let Some(file) = moved.map(|folder| folder.join("data.json")) {
        if let Ok(data) = std::fs::read(&file) {
            let _ = linkgroupz_core::write_atomic(&file, &linkgroupz_core::transfer::with_icon_images(&data, &data_directory));
        }
    }
    {
        let state = app.state::<AppState>();
        let mut settings = lock(&state.settings);
        let list = settings.profiles().into_iter().filter(|other| other.id != profile.id).collect();
        settings.profiles = list;
        settings.save();
    }
    notify(&app);
    crate::commands::remove_unused_icons(&app);
    Ok(())
}
