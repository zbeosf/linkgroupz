use linkgroupz_core::profiles::{self, Profile};
use linkgroupz_core::i18n::Language;
use linkgroupz_core::{write_atomic, GroupOpenMode, LinkStore, SearchScope};
use serde::{Deserialize, Serialize};
use std::fs;
use std::path::PathBuf;

/// 앱 설정. 앱 데이터 폴더의 settings.json 에 둔다.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct Settings {
    /// 그룹을 어떻게 열지 (기본 자동)
    pub group_open_mode: GroupOpenMode,
    /// 시작 안내를 본 적이 있는지 (있으면 기존 사용자)
    pub welcome_shown: bool,
    /// 지난번 켤 때의 앱 버전 ('바뀐 점'을 보여줄지 판단)
    pub last_seen_version: Option<String>,
    /// 1시간마다 새 버전이 있는지 스스로 확인할지 (기본 켬)
    pub auto_update_check: bool,
    /// 업데이트 창에서 '나중에'를 고른 버전
    pub update_skipped_version: Option<String>,
    /// 메뉴바(트레이) 창을 여닫는 전역 단축키 (기본 없음). 형식은 crates/app/src/shortcut.rs 참고
    pub popover_shortcut: Option<String>,
    /// 앱을 직접 켤 때 관리 창을 열지. 끄면(기본) 메뉴바(트레이) 창을 연다. 로그인할 때 자동으로 켜진 경우는 아무 창도 열지 않는다
    pub show_manager_on_launch: bool,
    /// Dock 에 아이콘을 보일지 (macOS, 기본 켬). 켜 두면 Dock · ⌘Tab 으로 돌아오고, Dock 아이콘을 누르면 관리 창이 열린다
    pub show_dock_icon: bool,
    /// 그룹 데이터를 하루에 한 번 백업해 며칠치 남길지 (0 이면 백업하지 않음, 기본 7)
    pub backup_days: u32,
    /// 사용자가 정한 메뉴바(트레이) 창의 크기 (폭, 높이). 없으면 기본 폭에 높이는 내용에 맞춘다
    pub popover_size: Option<(f64, f64)>,
    /// 메뉴바 창을 메뉴바에서 떼어 놓았는지. 떼어 놓으면 다른 곳을 눌러도 닫히지 않고, 끌어서 옮길 수 있다
    pub popover_detached: bool,
    /// 떼어 놓은 메뉴바 창의 자리 (화면 픽셀, 왼쪽 위)
    pub popover_position: Option<(i32, i32)>,
    /// 사용자가 바꾼 작은 미리보기 창의 크기 (폭, 높이). 없으면 기본 크기
    pub preview_size: Option<(f64, f64)>,
    /// 사용자가 바꾼 큰 미리보기 창의 크기 (폭, 높이). 없으면 기본 크기
    pub browser_size: Option<(f64, f64)>,
    /// 프로필 목록 (crates/core/src/profiles.rs). 비어 있으면 기본 프로필 하나. 쓸 때는 `profiles()` 로 고쳐 읽는다
    pub profiles: Vec<Profile>,
    /// 지금 프로필 id (기본 "default")
    pub current_profile: String,
    /// 검색할 곳 (이름 · 태그 · URL). 관리 창 · 메뉴바 · 보관함이 함께 쓴다
    pub search_scope: SearchScope,
    /// 화면 언어. 없으면 시스템 언어를 따른다 (한국어가 아니면 영어)
    pub language: Option<Language>,
    /// 화면 모드 (라이트 · 다크). 없으면 시스템 화면 모드를 따른다
    pub theme: Option<Appearance>,
}

/// 설정에서 고른 화면 모드
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum Appearance {
    Light,
    Dark,
}

impl Appearance {
    pub fn tauri_theme(self) -> tauri::Theme {
        match self {
            Appearance::Light => tauri::Theme::Light,
            Appearance::Dark => tauri::Theme::Dark,
        }
    }
}

impl Default for Settings {
    fn default() -> Self {
        Self {
            group_open_mode: GroupOpenMode::default(),
            welcome_shown: false,
            last_seen_version: None,
            auto_update_check: true,
            update_skipped_version: None,
            popover_shortcut: None,
            show_manager_on_launch: false,
            backup_days: linkgroupz_core::backups::DEFAULT_BACKUP_DAYS,
            show_dock_icon: true,
            popover_size: None,
            popover_detached: false,
            popover_position: None,
            preview_size: None,
            browser_size: None,
            profiles: Vec::new(),
            current_profile: profiles::DEFAULT_PROFILE_ID.to_owned(),
            search_scope: SearchScope::default(),
            language: None,
            theme: None,
        }
    }
}

impl Settings {
    /// 쓸 언어: 설정에서 고른 언어, 없으면 시스템 언어
    pub fn resolved_language(&self) -> Language {
        self.language.unwrap_or_else(|| sys_locale::get_locale().map_or(Language::En, |locale| Language::from_locale(&locale)))
    }

    /// 프로필 목록 (맨 앞은 늘 기본 프로필)
    pub fn profiles(&self) -> Vec<Profile> {
        profiles::normalized(&self.profiles)
    }

    /// 지금 프로필 (목록에 없는 id 면 기본 프로필)
    pub fn current_profile(&self) -> Profile {
        profiles::find(&self.profiles, &self.current_profile)
    }

    pub fn file_path() -> PathBuf {
        LinkStore::data_directory().join("settings.json")
    }

    /// 파일이 없거나 읽을 수 없으면 기본값
    pub fn load() -> Self {
        fs::read(Self::file_path()).ok().and_then(|data| serde_json::from_slice(&data).ok()).unwrap_or_default()
    }

    pub fn save(&self) {
        let path = Self::file_path();
        if let Some(folder) = path.parent() {
            let _ = fs::create_dir_all(folder);
        }
        if let Ok(data) = serde_json::to_vec_pretty(self) {
            let _ = write_atomic(&path, &data);
        }
    }
}
