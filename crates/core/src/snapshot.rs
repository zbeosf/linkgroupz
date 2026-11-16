use crate::base64url::Base64Url;
use crate::brand;
use crate::browser::Browser;
use crate::extension::ChromeExtensionDetector;
use crate::ids::Id;
use crate::models::{GroupColor, LinkGroup, SavedLink};
use crate::native::NativeSnapshotStore;
use crate::platform;
use crate::session_file::SessionFileReader;
use crate::tabs::{ChromeTab, ChromeTabGroupInfo, ChromeTabsReader, ScriptValue, TabImportCandidate, TabsError, TabsHost};
use serde::{Deserialize, Serialize};
use std::collections::{HashMap, HashSet};
use std::thread;
use std::time::{Duration, Instant};

/// AppleScript 가 돌려준 id · 번호를 스크립트에 다시 넣을 글자로. 숫자는 그대로, 글자는 따옴표로 감싼다.
/// 숫자 · 영문 · `-` 만 받아, 이상한 값이 스크립트에 섞이지 않게 한다.
fn script_literal(value: &ScriptValue) -> Option<String> {
    match value {
        ScriptValue::Int(number) => Some(number.to_string()),
        ScriptValue::Text(text) if !text.is_empty() && text.chars().all(|character| character.is_ascii_alphanumeric() || character == '-') => {
            Some(format!("\"{text}\""))
        }
        _ => None,
    }
}

/// 확장(chrome-extension/background.js 의 buildSnapshot)이 넘겨주는 모든 창의 탭 · 탭 그룹.
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeSnapshot {
    /// 앱이 요청해서 받은 결과에만 있다
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub nonce: Option<String>,
    /// 확장이 저장한 시각 (ms, JS Date.now())
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub saved_at: Option<f64>,
    /// 보낸 브라우저 (chrome · edge). 확장 1.3.0 부터
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub browser: Option<String>,
    pub windows: Vec<SnapshotWindow>,
    pub groups: Vec<SnapshotTabGroup>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotTab {
    pub title: String,
    pub url: String,
    /// 그룹에 없으면 -1
    pub group_id: i64,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
pub struct SnapshotWindow {
    pub id: i64,
    pub focused: bool,
    pub tabs: Vec<SnapshotTab>,
}

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct SnapshotTabGroup {
    pub id: i64,
    pub title: String,
    pub color: String,
    pub window_id: i64,
}

impl ChromeSnapshot {
    /// 확장이 결과를 실어 주는 URL 의 머리
    pub fn result_prefix() -> String {
        format!("https://{}/result#", brand::MARKER_HOST)
    }

    /// 확장이 결과 URL 로 바꿔 놓는 마커 탭 주소 (앱이 AppleScript 로 읽는다)
    pub fn snapshot_url(nonce: &str) -> String {
        format!("https://{}/snapshot#{nonce}", brand::MARKER_HOST)
    }

    /// 확장이 Native Messaging 으로 스냅샷을 보내고 스스로 닫는 마커 탭 주소
    pub fn push_url(nonce: &str) -> String {
        format!("https://{}/push#{nonce}", brand::MARKER_HOST)
    }

    /// 확장이 바꿔 놓은 결과 URL 을 읽는다. 다른 요청의 결과(nonce 불일치)면 None.
    pub fn decode(result_url: &str, nonce: &str) -> Option<ChromeSnapshot> {
        let encoded = result_url.strip_prefix(Self::result_prefix().as_str())?;
        let data = Base64Url::decode(encoded)?;
        let snapshot: ChromeSnapshot = serde_json::from_slice(&data).ok()?;
        (snapshot.nonce.as_deref() == Some(nonce)).then_some(snapshot)
    }

    /// 보낸 브라우저. 예전 확장처럼 값이 없으면 Chrome
    pub fn source_browser(&self) -> Browser {
        self.browser.as_deref().and_then(Browser::from_raw_value).unwrap_or(Browser::Chrome)
    }

    fn group_info_by_id(&self) -> HashMap<i64, ChromeTabGroupInfo> {
        self.groups
            .iter()
            .map(|group| {
                let color = GroupColor::from_raw_value(&group.color).unwrap_or(GroupColor::Grey);
                (group.id, ChromeTabGroupInfo::new(group.id, group.title.clone(), color))
            })
            .collect()
    }

    /// 맨 앞 창이 창 1 이 되도록 창 순서를 매긴다.
    pub fn tabs(&self) -> Vec<ChromeTab> {
        let infos = self.group_info_by_id();
        let ordered = self.windows.iter().filter(|w| w.focused).chain(self.windows.iter().filter(|w| !w.focused));
        let mut result = Vec::new();
        for (offset, window) in ordered.enumerate() {
            for tab in &window.tabs {
                result.push(ChromeTab {
                    window_index: offset + 1,
                    title: tab.title.clone(),
                    url: tab.url.clone(),
                    tab_group: infos.get(&tab.group_id).cloned(),
                });
            }
        }
        result
    }

    /// 탭이 하나 이상 있는 탭 그룹을 창 순서 · 탭 순서대로.
    pub fn tab_groups(&self) -> Vec<ChromeTabGroupImport> {
        let mut result: Vec<ChromeTabGroupImport> = Vec::new();
        for tab in self.tabs() {
            let Some(info) = tab.tab_group.clone() else {
                continue;
            };
            match result.iter_mut().find(|group| group.info.id == info.id) {
                Some(group) => group.tabs.push(tab),
                None => result.push(ChromeTabGroupImport { info, window_index: tab.window_index, tabs: vec![tab] }),
            }
        }
        result
    }
}

/// 그룹으로 가져올 브라우저 탭 그룹 하나.
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ChromeTabGroupImport {
    pub info: ChromeTabGroupInfo,
    pub window_index: usize,
    pub tabs: Vec<ChromeTab>,
}

impl ChromeTabGroupImport {
    pub fn id(&self) -> i64 {
        self.info.id
    }

    /// 그룹 안 탭을 고르는 단위. 열 수 없는 주소는 고를 수 없고, 같은 URL 은 하나만 남는다.
    /// existing(같은 이름의 기존 그룹)에 이미 있는 페이지는 '이미 있음'으로 고를 수 없다.
    pub fn candidates(&self, existing: Option<&LinkGroup>) -> Vec<TabImportCandidate> {
        TabImportCandidate::make(&self.tabs, existing.map_or(&[], |group| group.links.as_slice()))
    }

    /// 이름이 같은(앞뒤 공백 · 대소문자 무시) 기존 그룹. 여럿이면 목록에서 앞의 것.
    pub fn matching_group<'a>(&self, groups: &'a [LinkGroup]) -> Option<&'a LinkGroup> {
        let key = TabGroupImportPlan::name_key(self.info.display_title());
        groups.iter().find(|group| TabGroupImportPlan::name_key(&group.name) == key)
    }

    /// selected_ids(candidates 의 id)가 None 이면 고를 수 있는 탭을 모두 넣는다.
    pub fn make_link_group(&self, use_title_as_alias: bool, selected_ids: Option<&HashSet<usize>>) -> LinkGroup {
        let links = self
            .candidates(None)
            .iter()
            .filter(|candidate| selected_ids.is_none_or(|ids| ids.contains(&candidate.id)))
            .filter_map(|candidate| candidate.make_link(use_title_as_alias))
            .collect();
        LinkGroup::new(self.info.display_title()).with_color(self.info.color).with_links(links)
    }
}

/// 기존 그룹에 더할 페이지
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct PlanAddition {
    #[serde(rename = "groupID")]
    pub group_id: Id,
    pub links: Vec<SavedLink>,
}

/// 고른 탭 그룹 · 페이지로 무엇을 만들고 어디에 더할지.
///
/// 같은 이름의 기존 그룹이 있으면 새로 만들지 않고 그 그룹에 페이지를 더한다.
/// 같은 이름의 브라우저 탭 그룹이 여럿이면 하나의 그룹으로 모은다. 같은 URL 은 한 번만 넣는다.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct TabGroupImportPlan {
    pub new_groups: Vec<LinkGroup>,
    pub additions: Vec<PlanAddition>,
}

impl TabGroupImportPlan {
    pub fn page_count(&self) -> usize {
        self.new_groups.iter().map(|group| group.links.len()).sum::<usize>()
            + self.additions.iter().map(|addition| addition.links.len()).sum::<usize>()
    }

    pub fn is_empty(&self) -> bool {
        self.page_count() == 0
    }

    pub(crate) fn name_key(name: &str) -> String {
        name.trim().to_lowercase()
    }

    /// selections: 탭 그룹 id → 고른 candidates(existing) 의 id
    pub fn make(
        tab_groups: &[ChromeTabGroupImport],
        selections: &HashMap<i64, HashSet<usize>>,
        existing_groups: &[LinkGroup],
        use_title_as_alias: bool,
        browser: Browser,
    ) -> TabGroupImportPlan {
        let mut plan = TabGroupImportPlan::default();
        // 대상(기존 그룹 id 또는 새 그룹 이름) → 넣은 URL
        let mut seen_urls: HashMap<String, HashSet<String>> = HashMap::new();

        for tab_group in tab_groups {
            let Some(ids) = selections.get(&tab_group.id()).filter(|ids| !ids.is_empty()) else {
                continue;
            };
            let target = tab_group.matching_group(existing_groups);
            let title_key = Self::name_key(tab_group.info.display_title());
            let target_key = match target {
                Some(group) => format!("existing-{}", group.id),
                None => format!("new-{title_key}"),
            };
            let seen = seen_urls.entry(target_key).or_insert_with(|| {
                target
                    .map(|group| {
                        group.links.iter().map(|link| TabImportCandidate::normalized_key(link.url.as_str())).collect()
                    })
                    .unwrap_or_default()
            });
            let links: Vec<SavedLink> = tab_group
                .candidates(target)
                .iter()
                .filter(|candidate| ids.contains(&candidate.id) && candidate.is_selectable())
                .filter_map(|candidate| candidate.make_link(use_title_as_alias))
                .filter(|link| seen.insert(TabImportCandidate::normalized_key(link.url.as_str())))
                .collect();
            if links.is_empty() {
                continue;
            }

            if let Some(target) = target {
                match plan.additions.iter_mut().find(|addition| addition.group_id == target.id) {
                    Some(addition) => addition.links.extend(links),
                    None => plan.additions.push(PlanAddition { group_id: target.id, links }),
                }
            } else if let Some(group) = plan.new_groups.iter_mut().find(|group| Self::name_key(&group.name) == title_key) {
                group.links.extend(links);
            } else {
                // 가져온 브라우저로 열리게 한다
                plan.new_groups.push(
                    LinkGroup::new(tab_group.info.display_title())
                        .with_color(tab_group.info.color)
                        .with_links(links)
                        .with_browser(browser),
                );
            }
        }
        plan
    }
}

/// 가져오기 화면이 쓰는 탭 목록
#[derive(Clone, Debug, PartialEq)]
pub struct TabSourceResult {
    pub tabs: Vec<ChromeTab>,
    pub tab_groups: Vec<ChromeTabGroupImport>,
    /// 탭 그룹 정보를 못 읽은 이유
    pub note: Option<String>,
}

/// 가져오기 화면이 쓰는 탭 목록을 읽는다. 확장이 있으면 탭 그룹 정보까지, 없으면 (macOS) AppleScript 로 탭만 읽는다.
///
/// 기다리는 일이 있어 백그라운드 스레드에서 불러야 한다.
pub struct ChromeTabSource;

impl ChromeTabSource {
    /// 확장이 응답하기를 기다리는 시간
    pub const TIMEOUT: Duration = Duration::from_secs(5);
    const POLL_INTERVAL: Duration = Duration::from_millis(150);

    /// 탭 그룹 정보까지 있는 스냅샷 (그 브라우저에 LinkGroupz 확장이 있을 때).
    ///
    /// 1. 확장이 자동 저장한 파일({browser}-snapshot.json)이 지금 브라우저 실행 이후 것이면 그대로 쓴다. 브라우저를 건드리지 않는다.
    /// 2. 없으면 브라우저가 스스로 저장하는 세션 파일을 읽는다 (crates/core/src/session_file.rs). 이것도 브라우저를 건드리지 않는다.
    /// 3. 그것도 못 읽으면 마커 탭을 잠깐 열어 받아 오고, 앱을 다시 맨 앞으로 가져온다 (브라우저가 잠깐 앞으로 나온다).
    pub fn snapshot(browser: Browser, host: &dyn TabsHost) -> Result<ChromeSnapshot, TabsError> {
        if !browser.is_running() {
            return Err(TabsError::BrowserNotRunning(browser));
        }
        NativeSnapshotStore::install_if_possible();
        if let Some(snapshot) = NativeSnapshotStore::read_fresh(&NativeSnapshotStore::file_path(browser), browser.launch_date()) {
            return Ok(snapshot);
        }
        if let Some(snapshot) = SessionFileReader::read(browser) {
            return Ok(snapshot);
        }
        let result = if cfg!(target_os = "macos") {
            Self::snapshot_via_marker_tab(browser, host, Self::TIMEOUT)
        } else {
            Self::snapshot_via_push(browser, Self::TIMEOUT)
        };
        host.reactivate_app();
        result
    }

    pub fn load(browser: Browser, host: &dyn TabsHost) -> Result<TabSourceResult, TabsError> {
        if ChromeExtensionDetector::find(browser).is_none() {
            // 확장이 없어도 브라우저 세션 파일에서 탭 그룹까지 읽을 수 있다.
            if let Some(snapshot) = browser.is_running().then(|| SessionFileReader::read(browser)).flatten() {
                return Ok(TabSourceResult { tabs: snapshot.tabs(), tab_groups: snapshot.tab_groups(), note: None });
            }
            let note = crate::tr!("{} 에 LinkGroupz 확장이 없어 탭 그룹 없이 표시합니다.", "{} doesn't have the LinkGroupz extension, so tabs are shown without tab groups.", browser.display_name());
            return Self::tabs_without_groups(browser, host, note, TabsError::ExtensionRequired(browser));
        }
        match Self::snapshot(browser, host) {
            Ok(snapshot) => Ok(TabSourceResult { tabs: snapshot.tabs(), tab_groups: snapshot.tab_groups(), note: None }),
            Err(error @ TabsError::ExtensionNoResponse(_)) => {
                Self::tabs_without_groups(browser, host, error.to_string(), error)
            }
            Err(error) => Err(error),
        }
    }

    /// 확장 없이 탭만 읽는다. AppleScript 가 없는 운영체제에서는 `otherwise` 를 돌려준다.
    fn tabs_without_groups(
        browser: Browser,
        host: &dyn TabsHost,
        note: String,
        otherwise: TabsError,
    ) -> Result<TabSourceResult, TabsError> {
        if !cfg!(target_os = "macos") {
            // 브라우저가 꺼져 있으면 그 이유가 먼저다.
            return Err(if browser.is_running() { otherwise } else { TabsError::BrowserNotRunning(browser) });
        }
        let tabs = ChromeTabsReader::read(browser, host)?;
        Ok(TabSourceResult { tabs, tab_groups: Vec::new(), note: Some(note) })
    }

    /// 확장에게 탭 · 탭 그룹 목록을 받아 온다 (macOS).
    ///
    /// 맨 앞 창에 마커 탭(/snapshot#nonce)을 열면 확장이 그 탭을 결과 URL 로 바꾼다.
    /// 마커 탭은 열자마자 뒤로 돌리고, 결과를 읽은 뒤 닫는다. 탭을 열 때 브라우저가 잠깐 앞으로 나오는 것은 막을 수 없어 앱을 바로 다시 앞으로 가져온다.
    pub fn snapshot_via_marker_tab(browser: Browser, host: &dyn TabsHost, timeout: Duration) -> Result<ChromeSnapshot, TabsError> {
        if !browser.is_running() {
            return Err(TabsError::BrowserNotRunning(browser));
        }
        Self::read_marker_tab(browser, host, timeout)
    }

    /// 마커 탭을 열고 결과를 기다린다. 성공하든 실패하든 연 마커 탭은 닫는다.
    fn read_marker_tab(browser: Browser, host: &dyn TabsHost, timeout: Duration) -> Result<ChromeSnapshot, TabsError> {
        let nonce = Id::new().to_string();
        let tell = format!("tell application id \"{}\"", browser.bundle_identifier());
        let created = host.run_script(&format!(
            r#"{tell}
    if (count of windows) is 0 then error "no window" number 9001
    set w to window 1
    set oldIndex to active tab index of w
    set t to make new tab at end of tabs of w with properties {{URL:"{url}"}}
    set newID to id of t
    set active tab index of w to oldIndex
    return {{id of w, newID, oldIndex}}
end tell"#,
            url = ChromeSnapshot::snapshot_url(&nonce)
        ))?;
        // 브라우저는 다른 앱이 탭을 열면 맨 앞으로 나온다. 마커 탭은 위에서 바로 뒤로 돌렸고(확장은 뒤에 있는 탭도 처리한다),
        // 결과를 기다리는 동안 브라우저가 앞에 떠 있지 않게 앱을 바로 다시 앞으로 가져온다.
        host.reactivate_app();
        // 창 · 탭 id 는 Chrome · Whale 모두 글자(text)로 온다 (예전 Chrome 은 숫자).
        let parsed = created.as_list().and_then(|items| match items {
            [window, tab, old_index] => Some((script_literal(window)?, script_literal(tab)?, script_literal(old_index)?)),
            _ => None,
        });
        let Some((window_id, tab_id, old_index)) = parsed else {
            // 탭은 열렸을 수 있으니 마커 탭을 모두 닫는다.
            let _ = host.run_script(&Self::close_marker_tabs_script(&tell));
            return Err(TabsError::Script(crate::tr!("마커 탭을 열지 못했습니다", "Couldn't open the marker tab").to_owned()));
        };
        let tab_ref = format!("(first tab of (first window whose id is {window_id}) whose id is {tab_id})");

        let deadline = Instant::now() + timeout;
        let mut found = None;
        while found.is_none() && Instant::now() < deadline {
            thread::sleep(Self::POLL_INTERVAL);
            let Ok(value) = host.run_script(&format!("{tell} to get URL of {tab_ref}")) else {
                continue;
            };
            found = value.as_text().and_then(|url| ChromeSnapshot::decode(url, &nonce));
        }

        let _ = host.run_script(&format!(
            r#"{tell}
    try
        close {tab_ref}
    end try
    try
        set active tab index of (first window whose id is {window_id}) to {old_index}
    end try
end tell"#
        ));
        if found.is_none() {
            // 확장이 답하지 않았으면 이번 것과 예전에 남은 마커 탭을 모두 닫는다.
            let _ = host.run_script(&Self::close_marker_tabs_script(&tell));
        }
        found.ok_or(TabsError::ExtensionNoResponse(browser))
    }

    /// 마커 주소(https://<마커 호스트>/...)로 열린 탭을 모든 창에서 닫는다.
    fn close_marker_tabs_script(tell: &str) -> String {
        format!(
            r#"{tell}
    repeat with w in windows
        try
            close (every tab of w whose URL starts with "https://{host}/")
        end try
    end repeat
end tell"#,
            host = brand::MARKER_HOST
        )
    }

    /// 확장에게 탭 · 탭 그룹 목록을 받아 온다 (AppleScript 가 없는 Windows 등).
    ///
    /// 마커 주소(/push#nonce)를 브라우저로 열면 확장이 스냅샷을 Native Messaging 호스트로 보내고 그 탭을 닫는다.
    /// 호스트가 쓴 파일에 같은 nonce 가 적히기를 기다린다.
    pub fn snapshot_via_push(browser: Browser, timeout: Duration) -> Result<ChromeSnapshot, TabsError> {
        if !browser.is_running() {
            return Err(TabsError::BrowserNotRunning(browser));
        }
        let Some(app) = browser.app_path() else {
            return Err(TabsError::BrowserNotRunning(browser));
        };
        let nonce = Id::new().to_string();
        platform::open_urls(&app, &[ChromeSnapshot::push_url(&nonce)]).map_err(|error| TabsError::Script(error.to_string()))?;

        let file = NativeSnapshotStore::file_path(browser);
        let deadline = Instant::now() + timeout;
        while Instant::now() < deadline {
            thread::sleep(Self::POLL_INTERVAL);
            if let Some(snapshot) = NativeSnapshotStore::read(&file).filter(|s| s.nonce.as_deref() == Some(nonce.as_str())) {
                return Ok(snapshot);
            }
        }
        Err(TabsError::ExtensionNoResponse(browser))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::tabs::ScriptFailure;
    use std::sync::Mutex;

    /// 브라우저 대신 AppleScript 에 답하는 가짜. 마커 탭을 만들면 created 를 돌려주고,
    /// URL 을 물으면 (answer 이면) 확장이 바꿔 놓은 결과 주소를 준다.
    struct FakeBrowser {
        created: ScriptValue,
        answer: bool,
        scripts: Mutex<Vec<String>>,
    }

    impl FakeBrowser {
        fn new(created: ScriptValue, answer: bool) -> Self {
            Self { created, answer, scripts: Mutex::new(Vec::new()) }
        }

        fn scripts(&self) -> Vec<String> {
            self.scripts.lock().unwrap().clone()
        }
    }

    impl TabsHost for FakeBrowser {
        fn run_script(&self, source: &str) -> Result<ScriptValue, ScriptFailure> {
            self.scripts.lock().unwrap().push(source.to_owned());
            if source.contains("make new tab") {
                return Ok(self.created.clone());
            }
            if source.contains("get URL of") && self.answer {
                let nonce = source_nonce(&self.scripts()[0]);
                let payload = serde_json::json!({ "nonce": nonce, "browser": "chrome", "windows": [], "groups": [] });
                let url = format!("{}{}", ChromeSnapshot::result_prefix(), Base64Url::encode(payload.to_string().as_bytes()));
                return Ok(ScriptValue::Text(url));
            }
            Ok(ScriptValue::Null)
        }
    }

    fn source_nonce(script: &str) -> String {
        script.split("/snapshot#").nth(1).unwrap().split('"').next().unwrap().to_owned()
    }

    fn text(value: &str) -> ScriptValue {
        ScriptValue::Text(value.to_owned())
    }

    const SWEEP: &str = "whose URL starts with \"https://linkgroupz.invalid/\"";

    #[test]
    fn text_ids_from_chrome_and_whale() {
        // Chrome · Whale 의 AppleScript 는 창 · 탭 id 를 글자로 준다.
        let browser = FakeBrowser::new(ScriptValue::List(vec![text("1234"), text("5678"), ScriptValue::Int(2)]), true);
        let snapshot = ChromeTabSource::read_marker_tab(Browser::Whale, &browser, Duration::from_secs(2)).unwrap();
        assert!(snapshot.nonce.is_some());
        let scripts = browser.scripts();
        assert!(scripts.iter().any(|script| script.contains("close (first tab of (first window whose id is \"1234\") whose id is \"5678\")")), "연 마커 탭을 id 로 닫는다");
        assert!(scripts.iter().any(|script| script.contains("set active tab index of (first window whose id is \"1234\") to 2")));
        assert!(!scripts.iter().any(|script| script.contains(SWEEP)), "성공하면 다른 탭은 건드리지 않는다");
    }

    #[test]
    fn number_ids_from_older_chrome() {
        let browser = FakeBrowser::new(ScriptValue::List(vec![ScriptValue::Int(7), ScriptValue::Int(8), ScriptValue::Int(1)]), true);
        assert!(ChromeTabSource::read_marker_tab(Browser::Chrome, &browser, Duration::from_secs(2)).is_ok());
        assert!(browser.scripts().iter().any(|script| script.contains("close (first tab of (first window whose id is 7) whose id is 8)")));
    }

    #[test]
    fn marker_tabs_are_closed_when_reading_fails() {
        // id 를 읽지 못해도 열린 마커 탭을 남기지 않는다 (예전에는 Whale 에서 시도할 때마다 탭이 하나씩 남았다).
        let odd = FakeBrowser::new(ScriptValue::List(vec![text("1\" & do shell script \"x"), text("2"), ScriptValue::Int(1)]), true);
        assert!(matches!(ChromeTabSource::read_marker_tab(Browser::Whale, &odd, Duration::from_secs(1)), Err(TabsError::Script(_))));
        let scripts = odd.scripts();
        assert_eq!(scripts.len(), 2, "마커 탭 열기 → 마커 탭 모두 닫기");
        assert!(scripts[1].contains(SWEEP) && !scripts[1].contains("do shell script"), "이상한 id 는 스크립트에 넣지 않는다");

        // 확장이 답하지 않으면: 연 탭을 닫고, 예전에 남은 마커 탭도 닫는다.
        let silent = FakeBrowser::new(ScriptValue::List(vec![text("1"), text("2"), ScriptValue::Int(1)]), false);
        assert!(matches!(
            ChromeTabSource::read_marker_tab(Browser::Whale, &silent, Duration::from_millis(400)),
            Err(TabsError::ExtensionNoResponse(Browser::Whale))
        ));
        let scripts = silent.scripts();
        assert!(scripts.iter().any(|script| script.contains("close (first tab of (first window whose id is \"1\") whose id is \"2\")")));
        assert!(scripts.last().unwrap().contains(SWEEP));
    }

    #[test]
    fn literals() {
        assert_eq!(script_literal(&ScriptValue::Int(42)), Some("42".to_owned()));
        assert_eq!(script_literal(&text("AB-12")), Some("\"AB-12\"".to_owned()));
        assert_eq!(script_literal(&text("")), None);
        assert_eq!(script_literal(&text("1 or true")), None);
        assert_eq!(script_literal(&ScriptValue::Null), None);
    }
}
