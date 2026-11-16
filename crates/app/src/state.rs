use crate::commands::WhatsNew;
use crate::favicon_store::FaviconStore;
use crate::history::History;
use crate::preview::PreviewSessions;
use crate::settings::Settings;
use crate::update::UpdateController;
use linkgroupz_core::{Browser, LinkArchive, LinkGroup, LinkStore, Section};
use serde::Serialize;
use std::collections::HashMap;
use std::sync::{Arc, Mutex, MutexGuard};
use std::time::Instant;
use tauri::{AppHandle, Emitter, Manager};

/// 창들이 함께 쓰는 앱 상태
pub struct AppState {
    pub store: Mutex<LinkStore>,
    pub settings: Mutex<Settings>,
    pub favicons: FaviconStore,
    pub updater: Arc<UpdateController>,
    pub previews: PreviewSessions,
    /// 브라우저 앱 아이콘 (data URL). 설치돼 있지 않으면 None
    pub browser_icons: Mutex<HashMap<(Browser, u32), Option<String>>>,
    pub popover: PopoverState,
    /// '바뀐 점' 창에 보여 줄 내용
    pub whats_new: Mutex<Option<WhatsNew>>,
    /// 메뉴바 창에서 고른, 관리 창에서 이어서 할 일 (편집 · 삭제처럼 입력 · 확인이 필요한 것)
    pub manager_action: Mutex<Option<serde_json::Value>>,
    /// 되돌리기 · 다시 실행 기록
    pub history: History,
}

impl AppState {
    pub fn new(settings: Settings, updater: UpdateController) -> Self {
        Self {
            store: Mutex::new(LinkStore::open(settings.current_profile().data_file(&LinkStore::data_directory()))),
            settings: Mutex::new(settings),
            favicons: FaviconStore::new(),
            updater: Arc::new(updater),
            previews: PreviewSessions::new(),
            browser_icons: Mutex::new(HashMap::new()),
            popover: PopoverState::default(),
            whats_new: Mutex::new(None),
            manager_action: Mutex::new(None),
            history: History::default(),
        }
    }
}

/// 메뉴바(트레이) 창의 상태.
///
/// 이 창은 아이콘을 누를 때 만들고, 닫힌 뒤 한동안 쓰지 않으면 없앤다. 숨겨 둔 채로 계속 두면
/// 웹뷰 프로세스들이 남아, 메뉴바에만 떠 있을 때의 메모리가 몇 배가 된다.
#[derive(Default)]
pub struct PopoverState {
    /// 포커스를 잃어 닫힌 시각. 그 직후의 아이콘 클릭은 '닫기'로 본다.
    pub hidden_at: Mutex<Option<Instant>>,
    /// 창을 만드는 중이면, 화면이 내용 높이를 알려 올 때 띄울 자리(눌린 아이콘)
    pub pending_icon: Mutex<Option<tauri::Rect>>,
    /// 창을 띄우거나 닫을 때마다 올린다. 예약해 둔 '없애기'가 그 뒤에 창이 다시 쓰였는지 가린다.
    pub generation: std::sync::atomic::AtomicU64,
    /// 앱이 바꾸려는 창 크기. 이 크기로 바뀐 알림은 사용자가 끈 것이 아니다.
    pub expected_size: Mutex<Option<tauri::PhysicalSize<u32>>>,
    /// 사용자가 크기를 바꿀 때마다 올린다. 끄기를 멈춘 뒤에 한 번만 저장하려고.
    pub resize_generation: std::sync::atomic::AtomicU64,
}

/// 화면에 보내는 그룹 · 보관함 전체
#[derive(Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct StoreSnapshot {
    /// 화면 순서 (섹션 안 그룹은 섹션 자리에 이어서)
    pub groups: Vec<LinkGroup>,
    pub sections: Vec<Section>,
    /// 맨 위 단계 순서: "section:<id>" · "group:<id>"
    pub sidebar_order: Vec<String>,
    pub archive: LinkArchive,
    pub last_error: Option<String>,
    pub all_tags: Vec<String>,
}

impl StoreSnapshot {
    pub fn of(store: &LinkStore) -> Self {
        Self {
            groups: store.groups().to_vec(),
            sections: store.sections().to_vec(),
            sidebar_order: store.sidebar_order().iter().map(ToString::to_string).collect(),
            archive: store.archive().clone(),
            last_error: store.last_error().map(str::to_owned),
            all_tags: store.all_tags(),
        }
    }
}

/// 잠금이 오염돼도(다른 스레드가 잠근 채 패닉) 값은 그대로 쓴다.
pub fn lock<T>(mutex: &Mutex<T>) -> MutexGuard<'_, T> {
    mutex.lock().unwrap_or_else(|poisoned| poisoned.into_inner())
}

pub const STORE_CHANGED: &str = "store-changed";

/// 저장소를 바꾸고 되돌리기 기록에 남긴 뒤, 바뀐 내용을 모든 창에 알린다.
/// label 은 바꾸기 전 내용으로 무엇을 했는지 만든다 (예: ‘개발 문서’ 삭제). 되돌리기 알림에 보인다.
pub fn edit_store<T>(app: &AppHandle, label: impl FnOnce(&LinkStore) -> String, change: impl FnOnce(&mut LinkStore) -> T) -> T {
    let state = app.state::<AppState>();
    let (result, snapshot) = {
        let mut store = lock(&state.store);
        let label = label(&store);
        let before = store.document();
        let result = change(&mut store);
        state.history.record(store.file_path(), label, before, store.document());
        (result, StoreSnapshot::of(&store))
    };
    let _ = app.emit(STORE_CHANGED, snapshot);
    result
}

/// 저장소를 바꾸고, 바뀐 내용을 모든 창에 알린다. 되돌리기 기록에는 남기지 않는다 (섹션 접기 · 펴기처럼 화면 상태만 바뀌는 것).
pub fn mutate_store<T>(app: &AppHandle, change: impl FnOnce(&mut LinkStore) -> T) -> T {
    let state = app.state::<AppState>();
    let (result, snapshot) = {
        let mut store = lock(&state.store);
        let result = change(&mut store);
        (result, StoreSnapshot::of(&store))
    };
    let _ = app.emit(STORE_CHANGED, snapshot);
    result
}

pub fn read_store<T>(app: &AppHandle, read: impl FnOnce(&LinkStore) -> T) -> T {
    let state = app.state::<AppState>();
    let store = lock(&state.store);
    read(&store)
}
