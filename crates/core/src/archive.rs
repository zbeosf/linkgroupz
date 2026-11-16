use crate::ids::Id;
use crate::models::{GroupColor, LinkGroup, SavedLink};
use crate::search::SearchFilter;
use crate::sections::DropTarget;
use crate::store::{move_elements, LinkStore, MoveResult};
use crate::tabs::TabImportCandidate;
use crate::timestamp::Timestamp;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;

/// 보관함: 목록에서 치웠지만 지우지는 않은 그룹 · 페이지.
///
/// 보관한 그룹은 메뉴바 · 관리 창 목록과 검색에 나오지 않고, 보관함에서 복원하면 그룹 목록 끝으로 돌아간다.
/// 보관한 페이지는 어느 그룹에서 왔는지 기억해 두고, 복원할 때 그룹을 고르거나 새 그룹을 만든다.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct LinkArchive {
    pub groups: Vec<ArchivedGroup>,
    pub links: Vec<ArchivedLink>,
}

impl LinkArchive {
    pub fn is_empty(&self) -> bool {
        self.groups.is_empty() && self.links.is_empty()
    }

    pub fn count(&self) -> usize {
        self.groups.len() + self.links.len()
    }

    /// 보관함 검색. 관리 창 검색과 같은 규칙(SearchFilter)을 쓴다.
    /// - 보관 그룹: 그룹 이름 · 태그에 맞으면 그룹 전체, 아니면 맞는 페이지만 담아 남긴다.
    /// - 보관 페이지: 페이지 이름 · 주소, 또는 보관할 때 있던 그룹 이름에 맞으면 남긴다.
    pub fn filtered(&self, filter: &SearchFilter) -> LinkArchive {
        if filter.is_empty() {
            return self.clone();
        }
        let groups = self
            .groups
            .iter()
            .filter_map(|item| {
                let matched = filter.filter(&item.group)?;
                Some(ArchivedGroup { group: matched, archived_at: item.archived_at })
            })
            .collect();
        let links = self
            .links
            .iter()
            .filter(|item| {
                let mut source = LinkGroup::new(item.group_name.clone());
                source.links = vec![item.link.clone()];
                filter.filter(&source).is_some()
            })
            .cloned()
            .collect();
        LinkArchive { groups, links }
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchivedGroup {
    pub group: LinkGroup,
    pub archived_at: Timestamp,
}

impl ArchivedGroup {
    pub fn new(group: LinkGroup, archived_at: Timestamp) -> Self {
        Self { group, archived_at }
    }

    pub fn id(&self) -> Id {
        self.group.id
    }
}

#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct ArchivedLink {
    pub link: SavedLink,
    /// 보관할 때 있던 그룹. 그 그룹이 지워졌거나 보관됐을 수 있어 이름 · 색도 함께 둔다.
    #[serde(rename = "groupID")]
    pub group_id: Id,
    #[serde(rename = "groupName")]
    pub group_name: String,
    #[serde(rename = "groupColor")]
    pub group_color: GroupColor,
    #[serde(rename = "archivedAt")]
    pub archived_at: Timestamp,
}

impl ArchivedLink {
    pub fn new(link: SavedLink, from: &LinkGroup, archived_at: Timestamp) -> Self {
        Self { link, group_id: from.id, group_name: from.name.clone(), group_color: from.color, archived_at }
    }

    pub fn id(&self) -> Id {
        self.link.id
    }
}

impl LinkStore {
    /// 아이콘 이미지 정리 등에서 쓰는, 보관한 그룹까지 포함한 전체 그룹
    pub fn groups_including_archive(&self) -> Vec<LinkGroup> {
        self.groups.iter().cloned().chain(self.archive.groups.iter().map(|item| item.group.clone())).collect()
    }

    /// 그룹을 보관함으로 옮긴다. 최근에 보관한 것이 보관함 맨 위에 온다.
    pub fn archive_groups(&mut self, ids: &HashSet<Id>, at: Timestamp) {
        if ids.is_empty() {
            return;
        }
        self.mutate_all(|groups, archive| {
            let (moving, kept): (Vec<LinkGroup>, Vec<LinkGroup>) =
                std::mem::take(groups).into_iter().partition(|group| ids.contains(&group.id));
            *groups = kept;
            archive.groups.splice(0..0, moving.into_iter().map(|group| ArchivedGroup::new(group, at)));
        });
    }

    /// 그룹의 페이지를 보관함으로 옮긴다.
    pub fn archive_links(&mut self, ids: &HashSet<Id>, group_id: Id, at: Timestamp) {
        if ids.is_empty() {
            return;
        }
        self.mutate_all(|groups, archive| {
            let Some(group) = groups.iter_mut().find(|group| group.id == group_id) else {
                return;
            };
            let (moving, kept): (Vec<SavedLink>, Vec<SavedLink>) =
                std::mem::take(&mut group.links).into_iter().partition(|link| ids.contains(&link.id));
            group.links = kept;
            let origin = group.clone();
            archive.links.splice(0..0, moving.into_iter().map(|link| ArchivedLink::new(link, &origin, at)));
        });
    }

    /// 보관한 그룹을 그룹 목록 끝으로 되돌린다.
    pub fn restore_groups(&mut self, ids: &HashSet<Id>) {
        if ids.is_empty() {
            return;
        }
        self.mutate_all(|groups, archive| {
            let (restoring, kept): (Vec<ArchivedGroup>, Vec<ArchivedGroup>) =
                std::mem::take(&mut archive.groups).into_iter().partition(|item| ids.contains(&item.id()));
            archive.groups = kept;
            let existing: HashSet<Id> = groups.iter().map(|group| group.id).collect();
            for item in restoring {
                let mut group = item.group;
                // 같은 id 의 그룹이 이미 있으면(다른 기기에서 복사한 data.json 등) 새 id 로 넣어 둘 다 남긴다.
                if existing.contains(&group.id) {
                    group.id = Id::new();
                }
                // 그룹 목록 끝(섹션 밖)으로 돌아온다.
                group.section_id = None;
                groups.push(group);
            }
        });
    }

    /// 보관한 페이지를 그룹 끝으로 되돌린다. 그 그룹에 같은 주소가 이미 있으면 다시 넣지 않고 보관함에서만 뺀다.
    pub fn restore_links(&mut self, ids: &HashSet<Id>, target_id: Id) -> MoveResult {
        let mut result = MoveResult::default();
        if ids.is_empty() {
            return result;
        }
        self.mutate_all(|groups, archive| {
            let Some(target) = groups.iter_mut().find(|group| group.id == target_id) else {
                return;
            };
            let (restoring, kept): (Vec<ArchivedLink>, Vec<ArchivedLink>) =
                std::mem::take(&mut archive.links).into_iter().partition(|item| ids.contains(&item.id()));
            archive.links = kept;
            let mut existing: HashSet<String> =
                target.links.iter().map(|link| TabImportCandidate::normalized_key(link.url.as_str())).collect();
            // 보관함은 최근 것이 위라, 보관한 순서(오래된 것 먼저)로 넣는다.
            for item in restoring.into_iter().rev() {
                if existing.insert(TabImportCandidate::normalized_key(item.link.url.as_str())) {
                    target.links.push(item.link);
                    result.moved += 1;
                } else {
                    result.duplicates += 1;
                }
            }
        });
        result
    }

    /// 새 그룹을 만들고 보관한 페이지를 그 그룹으로 되돌린다.
    pub fn restore_links_to_new_group(&mut self, ids: &HashSet<Id>, group: LinkGroup) -> MoveResult {
        let group_id = self.add_group(group).id;
        self.restore_links(ids, group_id)
    }

    /// 보관한 그룹을 그룹 목록의 그 자리로 되돌린다 (보관함에서 왼쪽 그룹 목록으로 끌어 놓기).
    pub fn restore_groups_to(&mut self, ids: &[Id], target: DropTarget) {
        let wanted: HashSet<Id> = ids.iter().copied().collect();
        let before: HashSet<Id> = self.groups.iter().map(|group| group.id).collect();
        self.restore_groups(&wanted);
        // 되돌린 그룹 (같은 id 가 있어 새 id 를 받은 것 포함) — 끌던 순서대로
        let restored: Vec<Id> = self.groups.iter().map(|group| group.id).filter(|id| !before.contains(id)).collect();
        self.move_items(&restored, target);
    }

    /// 보관한 그룹 안의 페이지를 그룹 끝으로 되돌린다. 같은 주소가 이미 있으면 다시 넣지 않고 보관 그룹에서만 뺀다.
    pub fn restore_links_from_archived_group(&mut self, archived_group_id: Id, ids: &HashSet<Id>, target_id: Id) -> MoveResult {
        let mut result = MoveResult::default();
        self.mutate_all(|groups, archive| {
            let (Some(source), Some(target)) = (
                archive.groups.iter_mut().find(|item| item.id() == archived_group_id),
                groups.iter_mut().find(|group| group.id == target_id),
            ) else {
                return;
            };
            let (moving, kept): (Vec<SavedLink>, Vec<SavedLink>) = std::mem::take(&mut source.group.links).into_iter().partition(|link| ids.contains(&link.id));
            source.group.links = kept;
            let mut existing: HashSet<String> = target.links.iter().map(|link| TabImportCandidate::normalized_key(link.url.as_str())).collect();
            for link in moving {
                if existing.insert(TabImportCandidate::normalized_key(link.url.as_str())) {
                    target.links.push(link);
                    result.moved += 1;
                } else {
                    result.duplicates += 1;
                }
            }
        });
        result
    }

    /// 보관한 그룹 안의 페이지를 지운다 (되돌릴 수 없다).
    pub fn delete_links_in_archived_group(&mut self, archived_group_id: Id, ids: &HashSet<Id>) {
        self.mutate_all(|_, archive| {
            if let Some(item) = archive.groups.iter_mut().find(|item| item.id() == archived_group_id) {
                item.group.links.retain(|link| !ids.contains(&link.id));
            }
        });
    }

    /// 보관한 페이지를 보관한 그룹 안으로 넣는다 (끝에). 같은 주소가 이미 있으면 보관 페이지에서만 뺀다.
    pub fn move_archived_links_into_group(&mut self, ids: &HashSet<Id>, archived_group_id: Id) {
        self.mutate_all(|_, archive| {
            if !archive.groups.iter().any(|item| item.id() == archived_group_id) {
                return;
            }
            let (moving, kept): (Vec<ArchivedLink>, Vec<ArchivedLink>) = std::mem::take(&mut archive.links).into_iter().partition(|item| ids.contains(&item.id()));
            archive.links = kept;
            let target = archive.groups.iter_mut().find(|item| item.id() == archived_group_id).expect("위에서 확인");
            let mut existing: HashSet<String> = target.group.links.iter().map(|link| TabImportCandidate::normalized_key(link.url.as_str())).collect();
            for item in moving.into_iter().rev() {
                if existing.insert(TabImportCandidate::normalized_key(item.link.url.as_str())) {
                    target.group.links.push(item.link);
                }
            }
        });
    }

    /// 보관한 그룹끼리 순서를 바꾼다.
    pub fn move_archived_groups(&mut self, from_offsets: &[usize], to_offset: usize) {
        self.mutate_all(|_, archive| move_elements(&mut archive.groups, from_offsets, to_offset));
    }

    /// 보관한 페이지끼리 순서를 바꾼다.
    pub fn move_archived_links(&mut self, from_offsets: &[usize], to_offset: usize) {
        self.mutate_all(|_, archive| move_elements(&mut archive.links, from_offsets, to_offset));
    }

    /// 보관한 그룹 안 페이지의 순서를 바꾼다.
    pub fn move_links_in_archived_group(&mut self, archived_group_id: Id, from_offsets: &[usize], to_offset: usize) {
        self.mutate_all(|_, archive| {
            if let Some(item) = archive.groups.iter_mut().find(|item| item.id() == archived_group_id) {
                move_elements(&mut item.group.links, from_offsets, to_offset);
            }
        });
    }

    /// 섹션을 보관한다: 안의 그룹을 보관함으로 옮기고 섹션은 지운다 (보관함 버튼에 섹션을 끌어 놓기).
    pub fn archive_section(&mut self, section_id: Id, at: Timestamp) {
        let members: HashSet<Id> = self.groups.iter().filter(|group| group.section_id == Some(section_id)).map(|group| group.id).collect();
        // 섹션을 먼저 지우면 안의 그룹이 섹션 자리에 남으므로, 그룹을 보관한 뒤 빈 섹션을 지운다.
        self.archive_groups(&members, at);
        self.delete_section(section_id);
    }

    pub fn delete_archived_groups(&mut self, ids: &HashSet<Id>) {
        if ids.is_empty() {
            return;
        }
        self.mutate_all(|_, archive| archive.groups.retain(|item| !ids.contains(&item.id())));
    }

    pub fn delete_archived_links(&mut self, ids: &HashSet<Id>) {
        if ids.is_empty() {
            return;
        }
        self.mutate_all(|_, archive| archive.links.retain(|item| !ids.contains(&item.id())));
    }
}
