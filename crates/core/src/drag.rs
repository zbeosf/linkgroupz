use crate::brand;
use crate::ids::Id;

/// 페이지를 다른 그룹(사이드바)으로 끌어다 놓을 때 주고받는 내용.
/// 문자열로 실어 보내고, 이 형식이 아닌 문자열(다른 앱에서 끌어온 글 등)은 무시한다.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct LinkDragPayload {
    pub group_id: Id,
    pub link_ids: Vec<Id>,
}

impl LinkDragPayload {
    pub const PREFIX: &'static str = brand::DRAG_PAYLOAD_PREFIX;

    pub fn new(group_id: Id, link_ids: Vec<Id>) -> Self {
        Self { group_id, link_ids }
    }

    pub fn text(&self) -> String {
        let ids: Vec<String> = self.link_ids.iter().map(Id::to_string).collect();
        format!("{}{}|{}", Self::PREFIX, self.group_id, ids.join(","))
    }

    pub fn parse(text: &str) -> Option<Self> {
        let (group, links) = text.strip_prefix(Self::PREFIX)?.split_once('|')?;
        let group_id = Id::parse(group)?;
        let link_ids: Vec<Id> = links.split(',').filter_map(Id::parse).collect();
        if link_ids.is_empty() {
            return None;
        }
        Some(Self { group_id, link_ids })
    }
}
