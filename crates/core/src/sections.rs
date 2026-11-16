//! 섹션: 그룹을 묶어 접고 펼친다 (섹션 > 그룹 > 페이지).
//!
//! 저장 형식은 원본 LinkGroups 1.5.5 와 같다. 맨 위 단계(섹션 · 섹션 밖 그룹)의 순서는 `sidebarOrder` 에,
//! 그룹이 든 섹션은 그룹의 `sectionID` 에, 섹션 안 그룹의 순서는 `groups` 의 순서에 적는다.
//! `groups` 는 늘 화면에 보이는 순서로 맞춰 두어, 섹션을 모르는 이전 버전도 같은 순서로 보여 준다.

use crate::ids::Id;
use crate::models::{LinkGroup, Section, SidebarItem};
use crate::store::LinkStore;
use serde::Deserialize;
use std::collections::{HashMap, HashSet};

/// 끌어다 놓을 자리.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DropTarget {
    /// 넣을 섹션. 없으면 맨 위 단계(섹션 밖)
    pub section: Option<Id>,
    /// 이 항목 앞에 놓는다. 맨 위 단계면 섹션이나 섹션 밖 그룹, 섹션 안이면 그 섹션의 그룹
    pub before: Option<Id>,
    /// before 가 없을 때 이 항목 바로 뒤에 놓는다 (맨 위 단계). 둘 다 없으면 끝
    pub after: Option<Id>,
}

impl DropTarget {
    pub fn top_level_end() -> Self {
        Self::default()
    }

    pub fn into_section(section: Id) -> Self {
        Self { section: Some(section), ..Self::default() }
    }
}

impl LinkStore {
    /// 섹션 · 순서를 그룹 목록에 맞춘다: 없는 섹션을 가리키는 그룹은 섹션 밖으로, 순서에 빠진 항목은 끝에 더하고,
    /// 그룹 목록을 화면 순서로 다시 놓는다. 읽은 직후와 저장하기 직전에 부른다.
    pub(crate) fn normalize_sections(&mut self) {
        let mut seen = HashSet::new();
        self.sections.retain(|section| seen.insert(section.id));
        let section_ids: HashSet<Id> = self.sections.iter().map(|section| section.id).collect();
        for group in &mut self.groups {
            if group.section_id.is_some_and(|id| !section_ids.contains(&id)) {
                group.section_id = None;
            }
        }
        if self.sections.is_empty() {
            self.sidebar_order.clear();
            return;
        }

        let top_level: HashSet<Id> = self.groups.iter().filter(|group| group.section_id.is_none()).map(|group| group.id).collect();
        let mut placed = HashSet::new();
        let mut order: Vec<SidebarItem> = Vec::with_capacity(self.sidebar_order.len());
        for item in &self.sidebar_order {
            let valid = match item {
                SidebarItem::Section(id) => section_ids.contains(id),
                SidebarItem::Group(id) => top_level.contains(id),
            };
            if valid && placed.insert(item.id()) {
                order.push(*item);
            }
        }
        // 순서에 없는 항목(이전 버전이 더한 그룹 등)은 그룹 목록에서 처음 나오는 자리 순서로 끝에 더한다.
        for group in &self.groups {
            let item = match group.section_id {
                Some(section) => SidebarItem::Section(section),
                None => SidebarItem::Group(group.id),
            };
            if placed.insert(item.id()) {
                order.push(item);
            }
        }
        for section in &self.sections {
            if placed.insert(section.id) {
                order.push(SidebarItem::Section(section.id));
            }
        }

        // 그룹 목록을 화면 순서로: 맨 위 단계 순서대로, 섹션 자리에는 그 섹션의 그룹을 지금 순서대로.
        let mut in_section: HashMap<Id, Vec<LinkGroup>> = HashMap::new();
        let mut loose: Vec<Option<LinkGroup>> = Vec::new();
        let mut loose_index: HashMap<Id, usize> = HashMap::new();
        for group in std::mem::take(&mut self.groups) {
            match group.section_id {
                Some(section) => in_section.entry(section).or_default().push(group),
                None => {
                    loose_index.entry(group.id).or_insert(loose.len());
                    loose.push(Some(group));
                }
            }
        }
        let mut groups = Vec::with_capacity(loose.len() + in_section.values().map(Vec::len).sum::<usize>());
        for item in &order {
            match item {
                SidebarItem::Group(id) => {
                    if let Some(group) = loose_index.get(id).and_then(|index| loose[*index].take()) {
                        groups.push(group);
                    }
                }
                SidebarItem::Section(id) => groups.extend(in_section.remove(id).unwrap_or_default()),
            }
        }
        // 같은 id 가 두 번 있는 등 순서에 못 들어간 그룹도 잃지 않는다.
        groups.extend(loose.into_iter().flatten());
        groups.extend(in_section.into_values().flatten());
        self.groups = groups;
        self.sidebar_order = order;
    }

    /// 섹션이 하나도 없을 때 비워 둔 맨 위 순서를 그룹 순서로 채운다 (섹션을 처음 만들기 전에).
    fn ensure_sidebar_order(&mut self) {
        if self.sections.is_empty() {
            self.sidebar_order = self.groups.iter().map(|group| SidebarItem::Group(group.id)).collect();
        }
    }

    pub fn section(&self, id: Id) -> Option<&Section> {
        self.sections.iter().find(|section| section.id == id)
    }

    /// 섹션의 그룹 (화면 순서)
    pub fn groups_in_section(&self, id: Id) -> Vec<&LinkGroup> {
        self.groups.iter().filter(|group| group.section_id == Some(id)).collect()
    }

    /// 맨 위 단계에서 그 항목이 몇 번째인지. 섹션 안 그룹이면 그 섹션의 자리.
    fn top_level_index(&self, id: Id) -> Option<usize> {
        let id = self.group(id).and_then(|group| group.section_id).unwrap_or(id);
        self.sidebar_order.iter().position(|item| item.id() == id)
    }

    /// 화면에 보이는 순서의 번호 (여러 항목을 함께 옮길 때 원래 순서를 지키려고)
    fn display_ranks(&self) -> HashMap<Id, usize> {
        let mut ranks = HashMap::new();
        let mut rank = 0;
        for item in &self.sidebar_order {
            ranks.insert(item.id(), rank);
            rank += 1;
            if let SidebarItem::Section(section) = item {
                for group in self.groups.iter().filter(|group| group.section_id == Some(*section)) {
                    ranks.insert(group.id, rank);
                    rank += 1;
                }
            }
        }
        ranks
    }

    // MARK: - 섹션 만들기 · 바꾸기 · 지우기

    /// 섹션을 만든다. `at` 항목(맨 위 단계의 섹션 · 그룹. 섹션 안 그룹이면 그 섹션)의 자리에 놓고, 없으면 끝에 놓는다.
    /// `group_ids` 의 그룹을 화면 순서대로 넣는다.
    pub fn add_section(&mut self, name: &str, at: Option<Id>, group_ids: &[Id]) -> Section {
        let name = name.trim();
        let section = Section::new(if name.is_empty() { crate::tr!("새 섹션", "New Section") } else { name });
        self.normalize_sections();
        self.ensure_sidebar_order();
        let index = at.and_then(|id| self.top_level_index(id)).unwrap_or(self.sidebar_order.len());
        self.sections.push(section.clone());
        self.sidebar_order.insert(index, SidebarItem::Section(section.id));
        if group_ids.is_empty() {
            self.mutate_all(|_, _| {});
        } else {
            self.move_items(group_ids, DropTarget::into_section(section.id));
        }
        section
    }

    pub fn rename_section(&mut self, id: Id, name: &str) {
        let name = name.trim();
        if name.is_empty() {
            return;
        }
        if let Some(section) = self.sections.iter_mut().find(|section| section.id == id) {
            section.name = name.to_owned();
        }
        self.mutate_all(|_, _| {});
    }

    pub fn set_section_collapsed(&mut self, id: Id, collapsed: bool) {
        let Some(section) = self.sections.iter_mut().find(|section| section.id == id) else {
            return;
        };
        if section.is_collapsed == collapsed {
            return;
        }
        section.is_collapsed = collapsed;
        self.mutate_all(|_, _| {});
    }

    /// 섹션을 지운다. 안의 그룹은 지우지 않고 섹션이 있던 자리에 둔다.
    pub fn delete_section(&mut self, id: Id) {
        let Some(index) = self.sidebar_order.iter().position(|item| *item == SidebarItem::Section(id)) else {
            return;
        };
        let members: Vec<SidebarItem> =
            self.groups.iter().filter(|group| group.section_id == Some(id)).map(|group| SidebarItem::Group(group.id)).collect();
        self.sidebar_order.splice(index..=index, members);
        for group in &mut self.groups {
            if group.section_id == Some(id) {
                group.section_id = None;
            }
        }
        self.sections.retain(|section| section.id != id);
        self.mutate_all(|_, _| {});
    }

    /// 섹션을 맨 위 단계에서 한 칸 위(-1) · 아래(+1)로 옮긴다.
    pub fn move_section(&mut self, id: Id, offset: isize) {
        let Some(index) = self.sidebar_order.iter().position(|item| *item == SidebarItem::Section(id)) else {
            return;
        };
        let target = index.saturating_add_signed(offset).min(self.sidebar_order.len().saturating_sub(1));
        if target == index {
            return;
        }
        let item = self.sidebar_order.remove(index);
        self.sidebar_order.insert(target, item);
        self.mutate_all(|_, _| {});
    }

    // MARK: - 옮기기

    /// 그룹 · 섹션을 끌어다 놓는다. 여러 개면 화면에 보이던 순서를 지킨다. 섹션은 섹션 안에 넣을 수 없다
    /// (섹션을 다른 섹션 안에 놓으면 그 섹션 앞에 놓는다).
    pub fn move_items(&mut self, ids: &[Id], target: DropTarget) {
        if ids.is_empty() {
            return;
        }
        let moving: HashSet<Id> = ids.iter().copied().collect();
        let section_ids: HashSet<Id> = self.sections.iter().map(|section| section.id).collect();
        let into = target.section.filter(|section| section_ids.contains(section) && !moving.contains(section));

        // 섹션이 없고 섹션 안으로 넣는 것도 아니면, 그룹 목록 순서만 바꾸면 된다.
        if self.sections.is_empty() {
            let (moving_groups, mut kept): (Vec<LinkGroup>, Vec<LinkGroup>) =
                std::mem::take(&mut self.groups).into_iter().partition(|group| moving.contains(&group.id));
            let index = target
                .before
                .and_then(|id| kept.iter().position(|group| group.id == id))
                .or_else(|| target.after.and_then(|id| kept.iter().position(|group| group.id == id)).map(|index| index + 1))
                .unwrap_or(kept.len());
            kept.splice(index..index, moving_groups);
            self.groups = kept;
            self.mutate_all(|_, _| {});
            return;
        }

        self.normalize_sections();
        let ranks = self.display_ranks();
        let mut moving_sections: Vec<Id> = self
            .sidebar_order
            .iter()
            .filter_map(|item| match item {
                SidebarItem::Section(id) if moving.contains(id) => Some(*id),
                _ => None,
            })
            .collect();
        let moving_groups: Vec<Id> = self.groups.iter().filter(|group| moving.contains(&group.id)).map(|group| group.id).collect();
        self.sidebar_order.retain(|item| !moving.contains(&item.id()));

        match into {
            Some(section) => {
                // 섹션 안: 그룹 목록에서 그 섹션의 그룹들 사이 자리로 옮긴다 (섹션 안 순서는 그룹 목록 순서를 따른다).
                let (mut moved, mut kept): (Vec<LinkGroup>, Vec<LinkGroup>) =
                    std::mem::take(&mut self.groups).into_iter().partition(|group| moving.contains(&group.id));
                for group in &mut moved {
                    group.section_id = Some(section);
                }
                let index = target
                    .before
                    .and_then(|id| kept.iter().position(|group| group.id == id && group.section_id == Some(section)))
                    .or_else(|| kept.iter().rposition(|group| group.section_id == Some(section)).map(|index| index + 1))
                    .unwrap_or(kept.len());
                kept.splice(index..index, moved);
                self.groups = kept;
                // 섹션은 섹션 안에 넣지 못하므로 그 섹션 앞에 놓는다.
                if !moving_sections.is_empty() {
                    let index = self.sidebar_order.iter().position(|item| *item == SidebarItem::Section(section)).unwrap_or(0);
                    self.sidebar_order.splice(index..index, moving_sections.drain(..).map(SidebarItem::Section));
                }
            }
            None => {
                for group in &mut self.groups {
                    if moving.contains(&group.id) {
                        group.section_id = None;
                    }
                }
                let mut items: Vec<SidebarItem> = moving_sections
                    .iter()
                    .map(|id| SidebarItem::Section(*id))
                    .chain(moving_groups.iter().map(|id| SidebarItem::Group(*id)))
                    .collect();
                items.sort_by_key(|item| ranks.get(&item.id()).copied().unwrap_or(usize::MAX));
                let index = target
                    .before
                    .and_then(|id| self.sidebar_order.iter().position(|item| item.id() == id))
                    .or_else(|| {
                        target.after.and_then(|id| self.sidebar_order.iter().position(|item| item.id() == id)).map(|index| index + 1)
                    })
                    .unwrap_or(self.sidebar_order.len());
                self.sidebar_order.splice(index..index, items);
            }
        }
        self.mutate_all(|_, _| {});
    }
}
