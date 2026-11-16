use crate::models::{LinkGroup, SavedLink};
use serde::{Deserialize, Serialize};
use unicode_normalization::char::canonical_combining_class;
use unicode_normalization::UnicodeNormalization;

/// 검색할 곳 (검색창 옆 거르기 버튼). 이름은 그룹 이름과 페이지 이름(alias), 태그는 그룹 태그, URL 은 페이지 주소.
/// 모두 끄면 모두 켠 것으로 본다.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct SearchScope {
    pub name: bool,
    pub tags: bool,
    pub url: bool,
}

impl Default for SearchScope {
    fn default() -> Self {
        Self { name: true, tags: true, url: true }
    }
}

impl SearchScope {
    fn effective(self) -> Self {
        if self.name || self.tags || self.url {
            self
        } else {
            Self::default()
        }
    }
}

/// 검색어로 그룹과 링크를 거른다.
///
/// - 공백으로 나눈 검색어가 모두 맞아야 남는다 (대소문자 · 전각/반각 · 발음 구별 기호 무시).
/// - `#태그` 검색어는 그룹 태그만 본다.
/// - 그 밖의 검색어는 그룹 이름 · 태그에 맞으면 그룹의 모든 링크를, 아니면 alias · URL 에 맞는 링크만 남긴다.
/// - 검색할 곳(SearchScope)에서 끈 것은 보지 않는다.
#[derive(Clone, Debug, Default)]
pub struct SearchFilter {
    pub tag_terms: Vec<String>,
    pub text_terms: Vec<String>,
    pub scope: SearchScope,
}

impl SearchFilter {
    pub fn new(query: &str) -> Self {
        let mut tag_terms = Vec::new();
        let mut text_terms = Vec::new();
        for term in query.split_whitespace().map(Self::fold) {
            if term.starts_with('#') {
                let tag = term.trim_start_matches('#');
                if !tag.is_empty() {
                    tag_terms.push(tag.to_owned());
                }
            } else if !term.is_empty() {
                text_terms.push(term);
            }
        }
        Self { tag_terms, text_terms, scope: SearchScope::default() }
    }

    /// 검색할 곳을 정한다.
    pub fn with_scope(mut self, scope: SearchScope) -> Self {
        self.scope = scope.effective();
        self
    }

    /// 페이지 하나가 글자 검색어에 모두 맞는지 (그룹 안 검색). `#태그` 검색어는 보지 않는다.
    pub fn matches_link(&self, link: &SavedLink) -> bool {
        let text = self.link_text(link);
        self.text_terms.iter().all(|term| text.contains(term.as_str()))
    }

    fn link_text(&self, link: &SavedLink) -> String {
        let alias = if self.scope.name { link.alias.as_deref().unwrap_or("") } else { "" };
        let url = if self.scope.url { link.url.as_str() } else { "" };
        Self::fold(&format!("{alias} {url}"))
    }

    pub fn is_empty(&self) -> bool {
        self.tag_terms.is_empty() && self.text_terms.is_empty()
    }

    pub fn apply(&self, groups: &[LinkGroup]) -> Vec<LinkGroup> {
        if self.is_empty() {
            return groups.to_vec();
        }
        groups.iter().filter_map(|group| self.filter(group)).collect()
    }

    /// 그룹이 결과에 남으면 맞는 링크만 담아 돌려준다. 남지 않으면 None.
    pub fn filter(&self, group: &LinkGroup) -> Option<LinkGroup> {
        let tags: Vec<String> = group.tags.iter().map(|tag| Self::fold(tag)).collect();
        for tag_term in &self.tag_terms {
            if !tags.iter().any(|tag| tag.contains(tag_term.as_str())) {
                return None;
            }
        }

        let name = if self.scope.name { Self::fold(&group.name) } else { String::new() };
        let tag_text = if self.scope.tags { tags.join(" ") } else { String::new() };
        let group_text = format!("{name} {tag_text}");
        let remaining: Vec<&String> = self.text_terms.iter().filter(|term| !group_text.contains(term.as_str())).collect();
        if remaining.is_empty() {
            return Some(group.clone());
        }

        let links: Vec<_> = group
            .links
            .iter()
            .filter(|link| {
                let text = self.link_text(link);
                remaining.iter().all(|term| text.contains(term.as_str()))
            })
            .cloned()
            .collect();
        if links.is_empty() {
            return None;
        }
        let mut result = group.clone();
        result.links = links;
        Some(result)
    }

    /// 대소문자 · 전각/반각 · 발음 구별 기호(é → e)를 구별하지 않도록 접는다.
    ///
    /// 호환 분해(NFKD)로 전각 문자와 기호를 풀고, 붙는 기호를 뺀 뒤 다시 합친다(NFC).
    /// 다시 합치지 않으면 한글이 자모로 풀린 채 남아 "가" 가 "각" 에도 맞게 된다.
    pub fn fold(text: &str) -> String {
        text.nfkd().filter(|c| canonical_combining_class(*c) == 0).nfc().collect::<String>().to_lowercase()
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::LinkUrl;

    fn group() -> LinkGroup {
        let mut group = LinkGroup::new("업무 wiki");
        group.tags = vec!["daily".into()];
        group.links = vec![
            SavedLink::new(LinkUrl::new("https://jira.example.com/board"), Some("보드")),
            SavedLink::new(LinkUrl::new("https://wiki.example.com/home"), Some("팀 홈")),
        ];
        group
    }

    #[test]
    fn scope_limits_where_terms_match() {
        let all = SearchFilter::new("wiki");
        assert_eq!(all.filter(&group()).unwrap().links.len(), 2, "그룹 이름에 맞으면 그룹 전체");

        let url_only = SearchFilter::new("wiki").with_scope(SearchScope { name: false, tags: false, url: true });
        assert_eq!(url_only.filter(&group()).unwrap().links.len(), 1, "이름을 끄면 URL 에 맞는 페이지만");

        let name_only = SearchFilter::new("jira").with_scope(SearchScope { name: true, tags: false, url: false });
        assert!(name_only.filter(&group()).is_none(), "URL 을 끄면 주소로는 찾지 않는다");

        let tags_off = SearchFilter::new("daily").with_scope(SearchScope { name: true, tags: false, url: true });
        assert!(tags_off.filter(&group()).is_none(), "태그를 끄면 태그 글자로는 찾지 않는다");
        assert!(SearchFilter::new("#daily").with_scope(SearchScope { name: true, tags: false, url: true }).filter(&group()).is_some(), "#태그 는 늘 태그를 본다");

        let none = SearchFilter::new("jira").with_scope(SearchScope { name: false, tags: false, url: false });
        assert!(none.filter(&group()).is_some(), "모두 끄면 모두 켠 것으로 본다");
    }

    #[test]
    fn link_search_inside_group() {
        let filter = SearchFilter::new("홈 wiki");
        let links = group().links;
        assert!(!filter.matches_link(&links[0]));
        assert!(filter.matches_link(&links[1]), "이름 · URL 에 검색어가 모두 있으면 맞다");
    }
}
