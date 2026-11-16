//! 되돌리기 · 다시 실행 (macOS ⌘Z · ⌘⇧Z, Windows Ctrl+Z · Ctrl+Y · Ctrl+Shift+Z)
//!
//! 그룹 · 페이지 · 섹션 · 보관함을 바꿀 때마다(`state::edit_store`) 바꾸기 전과 후의 내용을 통째로 기억한다.
//! 데이터가 작아(수십 KB) 통째로 들고 있어도 부담이 없고, 어떤 변경이든 같은 방법으로 되돌린다.
//! - 프로필(데이터 파일)마다 따로, 최근 50단계까지. 앱을 끄면 사라진다 — 그 뒤는 자동 백업이 맡는다.
//! - 되돌린 뒤 새로 바꾸면 다시 실행할 기록은 버린다.
//! - 데이터 파일이 앱 밖에서 바뀌어 다시 읽으면 그 프로필의 기록을 비운다 (되돌리면 밖에서 바꾼 내용을 덮어쓰므로).
//! - 기록에 든 그룹의 아이콘 이미지는 되돌릴 수 있는 동안 지우지 않는다 (`commands::remove_unused_icons`).

use linkgroupz_core::tr;
use crate::state::{lock, AppState, StoreSnapshot, STORE_CHANGED};
use linkgroupz_core::{Id, LinkGroupsDocument, LinkStore};
use std::collections::{HashMap, HashSet, VecDeque};
use std::path::{Path, PathBuf};
use std::sync::Mutex;
use tauri::{AppHandle, Emitter, Manager};

/// 프로필마다 기억하는 단계 수
const LIMIT: usize = 50;

struct Entry {
    /// 무엇을 했는지 (예: ‘개발 문서’ 삭제)
    label: String,
    before: LinkGroupsDocument,
    after: LinkGroupsDocument,
}

#[derive(Default)]
struct Stacks {
    undo: VecDeque<Entry>,
    redo: Vec<Entry>,
}

#[derive(Default)]
pub struct History {
    /// 데이터 파일 경로 → 그 프로필의 기록
    stacks: Mutex<HashMap<PathBuf, Stacks>>,
}

#[derive(Clone, Copy)]
pub enum Step {
    Undo,
    Redo,
}

impl History {
    /// 바꾼 일을 기록한다. 내용이 그대로면(같은 자리로 옮기기 등) 남기지 않는다.
    pub fn record(&self, file: &Path, label: String, before: LinkGroupsDocument, after: LinkGroupsDocument) {
        if before == after {
            return;
        }
        let mut all = lock(&self.stacks);
        let stacks = all.entry(file.to_owned()).or_default();
        stacks.redo.clear();
        stacks.undo.push_back(Entry { label, before, after });
        while stacks.undo.len() > LIMIT {
            stacks.undo.pop_front();
        }
    }

    /// 그 프로필의 기록을 비운다.
    pub fn forget(&self, file: &Path) {
        lock(&self.stacks).remove(file);
    }

    /// 되돌리거나 다시 실행할 내용과 그 일의 이름을 꺼내고, 반대쪽 기록으로 옮긴다.
    fn take(&self, file: &Path, step: Step) -> Option<(String, LinkGroupsDocument)> {
        let mut all = lock(&self.stacks);
        let stacks = all.get_mut(file)?;
        match step {
            Step::Undo => {
                let entry = stacks.undo.pop_back()?;
                let result = (entry.label.clone(), entry.before.clone());
                stacks.redo.push(entry);
                Some(result)
            }
            Step::Redo => {
                let entry = stacks.redo.pop()?;
                let result = (entry.label.clone(), entry.after.clone());
                stacks.undo.push_back(entry);
                Some(result)
            }
        }
    }

    /// 기록에 든 그룹이 쓰는 아이콘 이미지 파일 이름
    pub fn icon_file_names(&self) -> HashSet<String> {
        let all = lock(&self.stacks);
        all.values()
            .flat_map(|stacks| stacks.undo.iter().chain(stacks.redo.iter()))
            .flat_map(|entry| [&entry.before, &entry.after])
            .flat_map(|document| document.groups.iter().chain(document.archive.groups.iter().map(|item| &item.group)))
            .filter_map(|group| group.icon.image_file_name().map(str::to_owned))
            .collect()
    }
}

/// 되돌리거나 다시 실행하고, 무엇을 했는지 돌려준다. 할 것이 없으면 None.
pub fn step(app: &AppHandle, step: Step) -> Option<String> {
    let state = app.state::<AppState>();
    let (label, snapshot) = {
        let mut store = lock(&state.store);
        let (label, document) = state.history.take(store.file_path(), step)?;
        store.replace_document(document);
        (label, StoreSnapshot::of(&store))
    };
    let _ = app.emit(STORE_CHANGED, snapshot);
    Some(label)
}

// MARK: - 무엇을 했는지 (알림에 쓰는 이름)

/// ‘그룹 이름’ 또는 '그룹 n개'
pub fn groups(store: &LinkStore, ids: &[Id]) -> String {
    match ids {
        [id] => store.group(*id).map_or_else(|| tr!("그룹", "group").to_owned(), |group| tr!("‘{}’", "“{}”", group.name)),
        _ => tr!("그룹 {}개", "{} groups", ids.len()),
    }
}

/// ‘보관한 그룹 이름’ 또는 '보관 그룹 n개'
pub fn archived_groups(store: &LinkStore, ids: &[Id]) -> String {
    match ids {
        [id] => store
            .archive()
            .groups
            .iter()
            .find(|item| item.id() == *id)
            .map_or_else(|| tr!("보관 그룹", "archived group").to_owned(), |item| tr!("‘{}’", "“{}”", item.group.name)),
        _ => tr!("보관 그룹 {}개", "{} archived groups", ids.len()),
    }
}

/// ‘페이지 이름’ 또는 '페이지 n개'
pub fn links(store: &LinkStore, group_id: Id, ids: &[Id]) -> String {
    match ids {
        [id] => store
            .group(group_id)
            .and_then(|group| group.links.iter().find(|link| link.id == *id))
            .map_or_else(|| tr!("페이지", "page").to_owned(), |link| tr!("‘{}’", "“{}”", link.display_name())),
        _ => tr!("페이지 {}개", "{} pages", ids.len()),
    }
}

/// ‘보관한 페이지 이름’ 또는 '보관 페이지 n개'
pub fn archived_links(store: &LinkStore, ids: &[Id]) -> String {
    match ids {
        [id] => store
            .archive()
            .links
            .iter()
            .find(|item| item.id() == *id)
            .map_or_else(|| tr!("보관 페이지", "archived page").to_owned(), |item| tr!("‘{}’", "“{}”", item.link.display_name())),
        _ => tr!("보관 페이지 {}개", "{} archived pages", ids.len()),
    }
}

/// ‘섹션 이름’ 섹션
pub fn section(store: &LinkStore, id: Id) -> String {
    store.sections().iter().find(|section| section.id == id).map_or_else(|| tr!("섹션", "section").to_owned(), |section| tr!("‘{}’ 섹션", "section “{}”", section.name))
}

#[cfg(test)]
mod tests {
    use super::*;
    use linkgroupz_core::{LinkArchive, LinkGroup};

    fn document(names: &[&str]) -> LinkGroupsDocument {
        LinkGroupsDocument::new(names.iter().map(|name| LinkGroup::new(*name)).collect(), LinkArchive::default())
    }

    #[test]
    fn undo_redo_and_limit() {
        let history = History::default();
        let file = Path::new("/profile/data.json");
        let (a, b, c) = (document(&["a"]), document(&["a", "b"]), document(&["a", "b", "c"]));
        history.record(file, "b 추가".into(), a.clone(), b.clone());
        history.record(file, "같음".into(), b.clone(), b.clone());
        history.record(file, "c 추가".into(), b.clone(), c.clone());

        assert_eq!(history.take(file, Step::Undo), Some(("c 추가".to_owned(), b.clone())), "내용이 그대로인 일은 남기지 않는다");
        assert_eq!(history.take(file, Step::Undo), Some(("b 추가".to_owned(), a.clone())));
        assert_eq!(history.take(file, Step::Undo), None);
        assert_eq!(history.take(file, Step::Redo), Some(("b 추가".to_owned(), b.clone())));
        assert!(history.take(Path::new("/other/data.json"), Step::Undo).is_none(), "프로필마다 따로");

        history.record(file, "새로 바꿈".into(), b.clone(), a.clone());
        assert_eq!(history.take(file, Step::Redo), None, "되돌린 뒤 새로 바꾸면 다시 실행할 기록은 버린다");

        for index in 0..LIMIT + 10 {
            history.record(file, format!("{index}"), document(&[]), document(&["x"]));
        }
        let mut count = 0;
        while history.take(file, Step::Undo).is_some() {
            count += 1;
        }
        assert_eq!(count, LIMIT);
        history.forget(file);
        assert!(history.take(file, Step::Redo).is_none(), "기록을 비운다");
    }
}

// MARK: - macOS 편집 메뉴

/// macOS 기본 메뉴의 '편집 → Undo · Redo'(⌘Z · ⇧⌘Z)는 키를 화면보다 먼저 가져가 글자 칸의 실행 취소만 한다.
/// 그 둘을 앱의 메뉴 항목으로 바꿔, 누르면 화면에 알린다(`EDIT_EVENT`, 다시 실행이면 true).
/// 포커스가 있는 창이 받아, 글자 칸이면 글자를(`native_edit`), 아니면 앱 되돌리기를 한다. Windows 는 메뉴가 없어 화면이 키를 바로 받는다.
#[cfg(target_os = "macos")]
pub mod edit_menu {
    use tauri::menu::{Menu, MenuEvent, MenuItem, MenuItemKind};
    use tauri::{AppHandle, Emitter, Wry};

    const UNDO: &str = "edit-undo";
    const REDO: &str = "edit-redo";
    pub const EDIT_EVENT: &str = "edit-undo-redo";

    pub fn app_menu(app: &AppHandle) -> tauri::Result<Menu<Wry>> {
        let menu = Menu::default(app)?;
        for item in menu.items()? {
            let Some(submenu) = item.as_submenu() else {
                continue;
            };
            let children = submenu.items()?;
            let is_undo_redo = |child: &MenuItemKind<Wry>| matches!(child, MenuItemKind::Predefined(item) if matches!(item.text().as_deref(), Ok("Undo" | "Redo")));
            if !children.iter().any(is_undo_redo) {
                continue;
            }
            for child in children.iter().filter(|child| is_undo_redo(child)) {
                submenu.remove(child)?;
            }
            submenu.insert(&MenuItem::with_id(app, UNDO, "Undo", true, Some("CmdOrCtrl+Z"))?, 0)?;
            submenu.insert(&MenuItem::with_id(app, REDO, "Redo", true, Some("CmdOrCtrl+Shift+Z"))?, 1)?;
        }
        Ok(menu)
    }

    pub fn on_menu_event(app: &AppHandle, event: MenuEvent) {
        match event.id().as_ref() {
            UNDO => {
                let _ = app.emit(EDIT_EVENT, false);
            }
            REDO => {
                let _ = app.emit(EDIT_EVENT, true);
            }
            _ => {}
        }
    }

    /// 글자 칸의 실행 취소 · 복귀 (메뉴가 원래 하던 일): 맨 앞 창의 응답 고리에 undo: · redo: 를 보낸다. 메인 스레드에서 부른다.
    pub fn send_native(redo: bool) {
        use objc2::runtime::{AnyObject, Sel};
        use objc2::{class, msg_send, sel};
        let action: Sel = if redo { sel!(redo:) } else { sel!(undo:) };
        let nothing: *const AnyObject = std::ptr::null();
        // Safety: NSApplication 의 공개 메서드를 메인 스레드에서 부른다. 받는 쪽이 없으면 아무 일도 하지 않는다.
        unsafe {
            let app: *mut AnyObject = msg_send![class!(NSApplication), sharedApplication];
            let _: bool = msg_send![app, sendAction: action, to: nothing, from: nothing];
        }
    }
}
