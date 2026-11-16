use crate::archive::LinkArchive;
use crate::brand;
use crate::fsutil::write_atomic;
use crate::ids::Id;
use crate::models::{LinkGroup, LinkGroupsDocument, SavedLink, Section, SidebarItem};
use crate::search::SearchFilter;
use crate::tabs::TabImportCandidate;
use serde::Serialize;
use serde_json::Value;
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

#[derive(Debug, thiserror::Error)]
enum LinkStoreError {
    #[error("{}", crate::tr!("지원하지 않는 데이터 파일 버전입니다: {}", "Unsupported data file version: {}", .0))]
    UnsupportedVersion(i64),
    #[error("{0}")]
    Io(#[from] std::io::Error),
    #[error("{0}")]
    Json(#[from] serde_json::Error),
}

/// 페이지를 옮기거나 복원한 결과
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Serialize)]
pub struct MoveResult {
    /// 대상 그룹에 새로 들어간 수
    pub moved: usize,
    /// 대상 그룹에 같은 주소가 이미 있어 원래 그룹에서만 뺀 수
    pub duplicates: usize,
}

/// 그룹 목록을 메모리에 들고, 바뀔 때마다 JSON 파일에 저장한다.
#[derive(Debug)]
pub struct LinkStore {
    /// 화면에 보이는 순서 (섹션 안 그룹은 섹션 자리에 이어서). 저장할 때마다 `normalize_sections` 가 맞춘다
    pub(crate) groups: Vec<LinkGroup>,
    pub(crate) archive: LinkArchive,
    pub(crate) sections: Vec<Section>,
    /// 맨 위 단계의 순서 (섹션 · 섹션 밖 그룹). 섹션이 하나도 없으면 비워 둔다
    pub(crate) sidebar_order: Vec<SidebarItem>,
    last_error: Option<String>,
    file_path: PathBuf,
    /// 마지막으로 읽거나 쓴 파일 내용. 파일이 이것과 달라졌을 때만 다시 읽는다.
    last_synced_data: Option<Vec<u8>>,
}

impl LinkStore {
    /// 앱 데이터 폴더. 환경 변수(`brand::env::DATA_DIR`)로 바꿀 수 있다 (사용법 스크린샷의 예시 데이터용).
    pub fn data_directory() -> PathBuf {
        if let Some(custom) = std::env::var_os(brand::env::DATA_DIR).filter(|value| !value.is_empty()) {
            return PathBuf::from(custom);
        }
        Self::default_data_directory()
    }

    /// 환경 변수와 상관없는 실제 앱 데이터 폴더
    pub fn default_data_directory() -> PathBuf {
        dirs::data_dir().unwrap_or_else(std::env::temp_dir).join(brand::DATA_DIR_NAME)
    }

    pub fn default_file_path() -> PathBuf {
        Self::data_directory().join("data.json")
    }

    /// 파일을 읽어 저장소를 만든다. 파일이 없으면 빈 목록으로 시작한다.
    pub fn open(file_path: impl Into<PathBuf>) -> Self {
        let mut store = Self {
            groups: Vec::new(),
            archive: LinkArchive::default(),
            sections: Vec::new(),
            sidebar_order: Vec::new(),
            last_error: None,
            file_path: file_path.into(),
            last_synced_data: None,
        };
        store.load();
        store
    }

    pub fn groups(&self) -> &[LinkGroup] {
        &self.groups
    }

    pub fn archive(&self) -> &LinkArchive {
        &self.archive
    }

    pub fn sections(&self) -> &[Section] {
        &self.sections
    }

    /// 맨 위 단계의 순서 (섹션이 없으면 그룹 순서 그대로)
    pub fn sidebar_order(&self) -> Vec<SidebarItem> {
        if self.sections.is_empty() {
            self.groups.iter().map(|group| SidebarItem::Group(group.id)).collect()
        } else {
            self.sidebar_order.clone()
        }
    }

    pub fn last_error(&self) -> Option<&str> {
        self.last_error.as_deref()
    }

    pub fn file_path(&self) -> &Path {
        &self.file_path
    }

    // MARK: - 파일

    fn read_document(data: &[u8]) -> Result<LinkGroupsDocument, LinkStoreError> {
        let document: LinkGroupsDocument = serde_json::from_slice(data)?;
        if document.version > LinkGroupsDocument::CURRENT_VERSION {
            return Err(LinkStoreError::UnsupportedVersion(document.version));
        }
        Ok(document)
    }

    pub fn load(&mut self) {
        if !self.file_path.exists() {
            self.groups = Vec::new();
            self.archive = LinkArchive::default();
            self.sections = Vec::new();
            self.sidebar_order = Vec::new();
            return;
        }
        let loaded = fs::read(&self.file_path)
            .map_err(LinkStoreError::from)
            .and_then(|data| Self::read_document(&data).map(|document| (data, document)));
        match loaded {
            Ok((data, document)) => {
                self.apply(document);
                self.last_synced_data = Some(data);
                self.last_error = None;
            }
            Err(error) => {
                // 읽지 못한 파일을 빈 목록으로 덮어쓰지 않도록 백업해 둔다.
                self.backup_unreadable_file();
                self.groups = Vec::new();
                self.archive = LinkArchive::default();
                self.sections = Vec::new();
                self.sidebar_order = Vec::new();
                self.last_error = Some(crate::tr!("데이터 파일을 읽지 못해 백업했습니다: {}", "Couldn't read the data file, so it was backed up: {}", error));
            }
        }
    }

    /// 파일이 마지막으로 읽거나 쓴 내용과 다르면 다시 읽는다. 읽을 수 없는 내용(쓰는 도중 등)이면 지금 목록을 그대로 둔다.
    /// 다시 읽었으면 true.
    pub fn reload_if_changed_on_disk(&mut self) -> bool {
        let Ok(data) = fs::read(&self.file_path) else {
            return false;
        };
        if self.last_synced_data.as_deref() == Some(data.as_slice()) {
            return false;
        }
        let Ok(document) = Self::read_document(&data) else {
            return false;
        };
        self.apply(document);
        self.last_synced_data = Some(data);
        true
    }

    fn apply(&mut self, document: LinkGroupsDocument) {
        self.groups = document.groups;
        self.archive = document.archive;
        self.sections = document.sections;
        self.sidebar_order = document.sidebar_order;
        self.normalize_sections();
    }

    fn backup_unreadable_file(&self) {
        let stamp = chrono::Local::now().format("%Y%m%d-%H%M%S");
        let backup = self.file_path.with_file_name(format!("data.broken-{stamp}.json"));
        let _ = fs::rename(&self.file_path, backup);
    }

    fn save(&mut self) {
        let result = (|| -> Result<Vec<u8>, LinkStoreError> {
            if let Some(folder) = self.file_path.parent() {
                fs::create_dir_all(folder)?;
            }
            self.normalize_sections();
            let mut document = LinkGroupsDocument::new(self.groups.clone(), self.archive.clone());
            document.sections = self.sections.clone();
            document.sidebar_order = self.sidebar_order.clone();
            let data = serde_json::to_vec_pretty(&sorted_keys(serde_json::to_value(&document)?))?;
            write_atomic(&self.file_path, &data)?;
            Ok(data)
        })();
        match result {
            Ok(data) => {
                self.last_synced_data = Some(data);
                self.last_error = None;
            }
            Err(error) => self.last_error = Some(crate::tr!("저장하지 못했습니다: {}", "Couldn't save: {}", error)),
        }
    }

    // MARK: - 되돌리기

    /// 지금 내용 통째로 (되돌리기 기록용)
    pub fn document(&self) -> LinkGroupsDocument {
        let mut document = LinkGroupsDocument::new(self.groups.clone(), self.archive.clone());
        document.sections = self.sections.clone();
        document.sidebar_order = self.sidebar_order.clone();
        document
    }

    /// 내용을 통째로 바꾸고 저장한다 (되돌리기 · 다시 실행).
    /// 섹션을 접고 편 상태는 되돌릴 일이 아니라서 지금 상태를 그대로 둔다.
    pub fn replace_document(&mut self, mut document: LinkGroupsDocument) {
        for section in &mut document.sections {
            if let Some(current) = self.sections.iter().find(|current| current.id == section.id) {
                section.is_collapsed = current.is_collapsed;
            }
        }
        self.apply(document);
        self.save();
    }

    fn mutate(&mut self, change: impl FnOnce(&mut Vec<LinkGroup>)) {
        change(&mut self.groups);
        self.save();
    }

    /// 그룹과 보관함을 함께 바꾼 뒤 한 번 저장한다 (보관 · 복원).
    pub(crate) fn mutate_all(&mut self, change: impl FnOnce(&mut Vec<LinkGroup>, &mut LinkArchive)) {
        change(&mut self.groups, &mut self.archive);
        self.save();
    }

    // MARK: - 조회

    pub fn group(&self, id: Id) -> Option<&LinkGroup> {
        self.groups.iter().find(|group| group.id == id)
    }

    /// 그룹들이 쓰는 태그 (대소문자만 다른 것은 하나로). 가나다 · 알파벳 순.
    pub fn all_tags(&self) -> Vec<String> {
        let mut seen = HashSet::new();
        let mut result: Vec<String> = self
            .groups
            .iter()
            .flat_map(|group| group.tags.iter())
            .filter(|tag| seen.insert(tag.to_lowercase()))
            .cloned()
            .collect();
        result.sort_by_cached_key(|tag| (SearchFilter::fold(tag), tag.clone()));
        result
    }

    // MARK: - 그룹

    pub fn add_group(&mut self, mut group: LinkGroup) -> LinkGroup {
        group.tags = LinkGroup::normalized_tags(&group.tags);
        let added = group.clone();
        self.mutate(|groups| groups.push(group));
        added
    }

    pub fn update_group(&mut self, group: LinkGroup) {
        self.mutate(|groups| {
            let Some(index) = groups.iter().position(|existing| existing.id == group.id) else {
                return;
            };
            let mut updated = group;
            updated.tags = LinkGroup::normalized_tags(&updated.tags);
            groups[index] = updated;
        });
    }

    /// 섹션을 바꾸면 그 섹션 끝으로(섹션에서 빼면 섹션이 있던 자리 바로 뒤로) 옮긴다.
    pub fn update_group_keeping_place(&mut self, group: LinkGroup) {
        let previous = self.group(group.id).and_then(|existing| existing.section_id);
        if previous == group.section_id {
            self.update_group(group);
            return;
        }
        let id = group.id;
        let target = group.section_id;
        self.update_group(LinkGroup { section_id: previous, ..group });
        self.move_items(&[id], crate::sections::DropTarget { section: target, before: None, after: previous });
    }

    pub fn delete_group(&mut self, id: Id) {
        self.mutate(|groups| groups.retain(|group| group.id != id));
    }

    pub fn delete_groups(&mut self, ids: &HashSet<Id>) {
        if ids.is_empty() {
            return;
        }
        self.mutate(|groups| groups.retain(|group| !ids.contains(&group.id)));
    }

    /// 그룹 목록(화면 순서) 기준으로 옮긴다. 섹션이 있으면 옮긴 결과대로 맨 위 순서도 다시 정한다.
    pub fn move_groups(&mut self, from_offsets: &[usize], to_offset: usize) {
        move_elements(&mut self.groups, from_offsets, to_offset);
        if !self.sections.is_empty() {
            let mut seen = HashSet::new();
            let mut order: Vec<SidebarItem> = self
                .groups
                .iter()
                .map(|group| group.section_id.map_or(SidebarItem::Group(group.id), SidebarItem::Section))
                .filter(|item| seen.insert(item.id()))
                .collect();
            // 빈 섹션은 원래 자리 앞의 항목 뒤에 둔다.
            for (index, item) in self.sidebar_order.iter().enumerate() {
                if seen.insert(item.id()) {
                    order.insert(index.min(order.len()), *item);
                }
            }
            self.sidebar_order = order;
        }
        self.mutate(|_| {});
    }

    // MARK: - 링크

    pub fn add_link(&mut self, link: SavedLink, group_id: Id) {
        self.add_links(vec![link], group_id);
    }

    pub fn add_links(&mut self, links: Vec<SavedLink>, group_id: Id) {
        if links.is_empty() {
            return;
        }
        self.mutate(|groups| {
            if let Some(group) = groups.iter_mut().find(|group| group.id == group_id) {
                group.links.extend(links);
            }
        });
    }

    pub fn update_link(&mut self, link: SavedLink, group_id: Id) {
        self.mutate(|groups| {
            let Some(group) = groups.iter_mut().find(|group| group.id == group_id) else {
                return;
            };
            let Some(existing) = group.links.iter_mut().find(|existing| existing.id == link.id) else {
                return;
            };
            let mut updated = link;
            updated.alias = SavedLink::normalized_alias(updated.alias.as_deref());
            *existing = updated;
        });
    }

    /// 그룹의 페이지 목록을 통째로 바꾼다 (그룹 동기화)
    pub fn set_group_links(&mut self, group_id: Id, links: Vec<SavedLink>) {
        self.mutate(|groups| {
            if let Some(group) = groups.iter_mut().find(|group| group.id == group_id) {
                group.links = links;
            }
        });
    }

    /// 그룹을 열 때 이 페이지도 열지 (페이지 줄의 스위치)
    pub fn set_link_opens_with_group(&mut self, group_id: Id, link_id: Id, opens: bool) {
        self.mutate(|groups| {
            if let Some(link) = groups.iter_mut().find(|group| group.id == group_id).and_then(|group| group.links.iter_mut().find(|link| link.id == link_id)) {
                link.opens_with_group = opens;
            }
        });
    }

    pub fn delete_link(&mut self, id: Id, group_id: Id) {
        self.delete_links(&HashSet::from([id]), group_id);
    }

    pub fn delete_links(&mut self, ids: &HashSet<Id>, group_id: Id) {
        if ids.is_empty() {
            return;
        }
        self.mutate(|groups| {
            if let Some(group) = groups.iter_mut().find(|group| group.id == group_id) {
                group.links.retain(|link| !ids.contains(&link.id));
            }
        });
    }

    /// 페이지를 다른 그룹 끝으로 옮긴다. 원래 순서를 지킨다.
    /// 대상 그룹에 같은 주소가 이미 있으면 다시 넣지 않고 원래 그룹에서만 뺀다.
    pub fn move_links(&mut self, ids: &HashSet<Id>, source_id: Id, target_id: Id) -> MoveResult {
        let mut result = MoveResult::default();
        if source_id == target_id || ids.is_empty() {
            return result;
        }
        self.mutate(|groups| {
            let (Some(source), Some(target)) = (
                groups.iter().position(|group| group.id == source_id),
                groups.iter().position(|group| group.id == target_id),
            ) else {
                return;
            };
            let (moving, kept): (Vec<SavedLink>, Vec<SavedLink>) =
                std::mem::take(&mut groups[source].links).into_iter().partition(|link| ids.contains(&link.id));
            groups[source].links = kept;
            let mut existing: HashSet<String> =
                groups[target].links.iter().map(|link| TabImportCandidate::normalized_key(link.url.as_str())).collect();
            for link in moving {
                if existing.insert(TabImportCandidate::normalized_key(link.url.as_str())) {
                    groups[target].links.push(link);
                    result.moved += 1;
                } else {
                    result.duplicates += 1;
                }
            }
        });
        result
    }

    pub fn move_links_within(&mut self, group_id: Id, from_offsets: &[usize], to_offset: usize) {
        self.mutate(|groups| {
            if let Some(group) = groups.iter_mut().find(|group| group.id == group_id) {
                move_elements(&mut group.links, from_offsets, to_offset);
            }
        });
    }
}

/// 목록의 항목들을 한꺼번에 옮긴다. `to_offset` 은 옮기기 전 목록 기준의 끼워 넣을 자리다
/// (SwiftUI 의 `move(fromOffsets:toOffset:)` 와 같은 의미).
pub fn move_elements<T>(items: &mut Vec<T>, from_offsets: &[usize], to_offset: usize) {
    let mut sources: Vec<usize> = from_offsets.iter().copied().filter(|index| *index < items.len()).collect();
    sources.sort_unstable();
    sources.dedup();
    if sources.is_empty() {
        return;
    }
    let before = sources.iter().filter(|index| **index < to_offset).count();
    let mut moving: Vec<T> = sources.iter().rev().map(|index| items.remove(*index)).collect();
    moving.reverse();
    let insert_at = to_offset.saturating_sub(before).min(items.len());
    items.splice(insert_at..insert_at, moving);
}

/// JSON 객체의 키를 이름순으로 다시 놓는다. 파일을 비교 · 병합하기 쉽도록 항상 같은 순서로 쓴다.
pub(crate) fn sorted_keys(value: Value) -> Value {
    match value {
        Value::Object(map) => {
            let mut entries: Vec<(String, Value)> = map.into_iter().collect();
            entries.sort_by(|a, b| a.0.cmp(&b.0));
            Value::Object(entries.into_iter().map(|(key, value)| (key, sorted_keys(value))).collect())
        }
        Value::Array(items) => Value::Array(items.into_iter().map(sorted_keys).collect()),
        other => other,
    }
}
