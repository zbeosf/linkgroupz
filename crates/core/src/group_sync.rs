//! 그룹 동기화: 그룹을 브라우저에 같은 이름으로 열린 탭 그룹과 비교해 맞춘다.
//!
//! 탭 그룹 전체로 덮어쓰거나, 브라우저에만 있는 페이지를 골라 더하고 이 그룹에만 있는 페이지를 골라 뺀다.
//! 같은 페이지인지는 주소로 본다 (`#` 뒤 · 끝의 `/` · 대소문자는 가리지 않는다, 열린 탭 가져오기와 같은 규칙).

use crate::ids::Id;
use crate::models::{LinkUrl, SavedLink};
use crate::search::SearchFilter;
use crate::snapshot::ChromeTabGroupImport;
use crate::tabs::TabImportCandidate;
use serde::{Deserialize, Serialize};
use std::collections::HashSet;
use url::Url;

/// 탭 그룹의 페이지 하나
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SyncTab {
    pub url: String,
    pub title: String,
}

/// 그룹과 탭 그룹을 비교한 결과
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct SyncComparison {
    /// 브라우저에만 있는 페이지 (탭 목록의 번호)
    pub browser_only: Vec<usize>,
    /// 이 그룹에만 있는 페이지
    pub here_only: Vec<Id>,
    /// 양쪽에 다 있는 페이지 수
    pub same: usize,
}

fn key(url: &str) -> String {
    TabImportCandidate::normalized_key(url)
}

/// 그룹으로 담을 수 있는 주소인지 (http · https · file)
fn supported(url: &str) -> bool {
    Url::parse(url).is_ok_and(|url| ["http", "https", "file"].contains(&url.scheme()))
}

/// 그룹 이름과 같은 이름(대소문자 · 앞뒤 공백 무시)의 탭 그룹. 여러 개면 맨 앞 창의 것.
pub fn find_tab_group<'a>(tab_groups: &'a [ChromeTabGroupImport], name: &str) -> Option<&'a ChromeTabGroupImport> {
    let wanted = SearchFilter::fold(name.trim());
    tab_groups.iter().filter(|group| SearchFilter::fold(group.info.title.trim()) == wanted).min_by_key(|group| group.window_index)
}

/// 탭 그룹의 페이지 (그룹으로 담을 수 없는 주소 · 같은 주소는 뺀다)
pub fn tabs_of(tab_group: &ChromeTabGroupImport) -> Vec<SyncTab> {
    let mut seen = HashSet::new();
    tab_group
        .tabs
        .iter()
        .filter(|tab| supported(&tab.url) && seen.insert(key(&tab.url)))
        .map(|tab| SyncTab { url: tab.url.clone(), title: tab.title.clone() })
        .collect()
}

pub fn compare(links: &[SavedLink], tabs: &[SyncTab]) -> SyncComparison {
    let here: HashSet<String> = links.iter().map(|link| key(link.url.as_str())).collect();
    let there: HashSet<String> = tabs.iter().map(|tab| key(&tab.url)).collect();
    SyncComparison {
        browser_only: tabs.iter().enumerate().filter(|(_, tab)| !here.contains(&key(&tab.url))).map(|(index, _)| index).collect(),
        here_only: links.iter().filter(|link| !there.contains(&key(link.url.as_str()))).map(|link| link.id).collect(),
        same: links.iter().filter(|link| there.contains(&key(link.url.as_str()))).count(),
    }
}

fn new_link(tab: &SyncTab, use_title_as_alias: bool) -> SavedLink {
    let title = tab.title.trim();
    SavedLink::new(LinkUrl::new(tab.url.clone()), (use_title_as_alias && !title.is_empty()).then_some(title))
}

/// 탭 그룹 전체로 덮어쓴 페이지 목록 (탭 순서대로). 이미 있던 페이지는 이름 · 스위치를 그대로 둔다.
pub fn replaced(links: &[SavedLink], tabs: &[SyncTab], use_title_as_alias: bool) -> Vec<SavedLink> {
    tabs.iter()
        .map(|tab| links.iter().find(|link| key(link.url.as_str()) == key(&tab.url)).cloned().unwrap_or_else(|| new_link(tab, use_title_as_alias)))
        .collect()
}

/// 고른 페이지를 더하고(끝에) 뺀 페이지 목록
pub fn merged(links: &[SavedLink], tabs: &[SyncTab], add: &[usize], remove: &HashSet<Id>, use_title_as_alias: bool) -> Vec<SavedLink> {
    let mut result: Vec<SavedLink> = links.iter().filter(|link| !remove.contains(&link.id)).cloned().collect();
    for tab in add.iter().filter_map(|index| tabs.get(*index)) {
        if !result.iter().any(|link| key(link.url.as_str()) == key(&tab.url)) {
            result.push(new_link(tab, use_title_as_alias));
        }
    }
    result
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::models::GroupColor;
    use crate::tabs::{ChromeTab, ChromeTabGroupInfo};

    fn link(url: &str, alias: &str) -> SavedLink {
        SavedLink::new(LinkUrl::new(url), Some(alias))
    }

    fn tab(url: &str, title: &str) -> SyncTab {
        SyncTab { url: url.into(), title: title.into() }
    }

    fn tab_group(title: &str, window_index: usize, urls: &[&str]) -> ChromeTabGroupImport {
        let info = ChromeTabGroupInfo::new(1, title, GroupColor::Blue);
        ChromeTabGroupImport {
            info: info.clone(),
            window_index,
            tabs: urls.iter().map(|url| ChromeTab { window_index, title: "t".into(), url: (*url).into(), tab_group: Some(info.clone()) }).collect(),
        }
    }

    #[test]
    fn finds_tab_group_by_name() {
        let groups = [tab_group("다른", 1, &[]), tab_group(" 업무 ", 2, &[]), tab_group("업무", 1, &[])];
        assert_eq!(find_tab_group(&groups, "업무").unwrap().window_index, 1, "같은 이름이 여럿이면 맨 앞 창");
        assert!(find_tab_group(&groups, "없음").is_none());
        let tabs = tabs_of(&tab_group("업무", 1, &["https://a.com/", "chrome://settings", "https://a.com", "https://b.com"]));
        assert_eq!(tabs.iter().map(|tab| tab.url.as_str()).collect::<Vec<_>>(), ["https://a.com/", "https://b.com"], "담을 수 없는 주소 · 같은 주소는 뺀다");
    }

    #[test]
    fn compare_and_apply() {
        let mut off = link("https://a.com/x", "끈 페이지");
        off.opens_with_group = false;
        let links = vec![off.clone(), link("https://only-here.com", "여기만")];
        let tabs = vec![tab("https://new.com", "새 탭"), tab("https://A.com/x/#top", "A 제목")];

        let comparison = compare(&links, &tabs);
        assert_eq!(comparison.browser_only, [0]);
        assert_eq!(comparison.here_only, [links[1].id]);
        assert_eq!(comparison.same, 1, "# 뒤 · 끝의 / · 대소문자는 가리지 않는다");

        let all = replaced(&links, &tabs, true);
        assert_eq!(all.len(), 2);
        assert_eq!(all[0].alias.as_deref(), Some("새 탭"), "탭 순서대로, 새 페이지는 탭 제목을 이름으로");
        assert_eq!(all[1], off, "이미 있던 페이지는 이름 · 스위치를 그대로");

        let chosen = merged(&links, &tabs, &[0, 1], &HashSet::from([links[1].id]), false);
        assert_eq!(chosen.iter().map(|link| link.url.as_str()).collect::<Vec<_>>(), ["https://a.com/x", "https://new.com"], "고른 것만 빼고 더한다 (이미 있으면 다시 넣지 않는다)");
        assert_eq!(chosen[1].alias, None, "탭 제목을 이름으로 쓰지 않으면 비워 둔다");
    }
}
