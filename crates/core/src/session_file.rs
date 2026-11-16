//! 브라우저가 스스로 저장하는 세션 파일(SNSS)에서 지금 열린 창 · 탭 · 탭 그룹을 읽는다.
//!
//! Chrome · Edge · Whale 은 창과 탭이 바뀔 때마다 몇 초 안에 프로필 폴더의 `Sessions/Session_<시각>` 에
//! 바뀐 내용을 명령으로 덧붙여 쓴다(다시 켤 때 복원하려고). 이 파일을 읽으면 브라우저에 탭을 열지 않아도
//! (그래서 브라우저가 맨 앞으로 나오지 않고) 탭 그룹까지 알 수 있다.
//! 형식은 Chromium 의 components/sessions/core/session_service_commands.cc · base/pickle.cc 를 따른다.

use crate::brand;
use crate::browser::Browser;
use crate::extension::ChromeExtensionDetector;
use crate::snapshot::{ChromeSnapshot, SnapshotTab, SnapshotTabGroup, SnapshotWindow};
use crate::timestamp::Timestamp;
use std::collections::{BTreeMap, HashMap};
use std::fs;
use std::path::{Path, PathBuf};

/// 세션 파일 명령 번호 (session_service_commands.cc)
mod command {
    pub const SET_TAB_WINDOW: u8 = 0;
    pub const SET_TAB_INDEX_IN_WINDOW: u8 = 2;
    pub const PRUNED_FROM_BACK: u8 = 5;
    pub const UPDATE_TAB_NAVIGATION: u8 = 6;
    pub const SET_SELECTED_NAVIGATION_INDEX: u8 = 7;
    pub const SET_WINDOW_TYPE: u8 = 9;
    pub const PRUNED_FROM_FRONT: u8 = 11;
    pub const TAB_CLOSED: u8 = 16;
    pub const WINDOW_CLOSED: u8 = 17;
    pub const SET_ACTIVE_WINDOW: u8 = 20;
    pub const NAVIGATION_PATH_PRUNED: u8 = 24;
    pub const SET_TAB_GROUP: u8 = 25;
    pub const SET_TAB_GROUP_METADATA: u8 = 27;
}

/// 일반 창 (팝업 · 앱 창은 뺀다)
const NORMAL_WINDOW: i32 = 0;

/// 탭 그룹 색 번호 → 확장이 쓰는 이름 (tab_groups::TabGroupColorId)
const COLORS: [&str; 9] = ["grey", "blue", "red", "yellow", "green", "pink", "purple", "cyan", "orange"];

#[derive(Default)]
struct TabState {
    window: Option<i32>,
    index: i32,
    /// 탐색 기록 번호 → (주소, 제목)
    navigations: BTreeMap<i32, (String, String)>,
    selected: Option<i32>,
    group: Option<u128>,
}

/// 세션 파일 하나(프로필 하나)를 처음부터 끝까지 적용한 상태
#[derive(Default)]
pub struct Session {
    tabs: HashMap<i32, TabState>,
    window_types: HashMap<i32, i32>,
    /// 그룹 → (이름, 색 번호)
    groups: HashMap<u128, (String, u32)>,
    active_window: Option<i32>,
    /// 명령이 나온 순서대로의 창 (같은 순서로 보여 준다)
    window_order: Vec<i32>,
}

impl Session {
    /// 파일 내용을 읽는다. SNSS 파일이 아니거나 암호화된 파일이면 None
    pub fn parse(data: &[u8]) -> Option<Self> {
        if data.len() < 8 || &data[..4] != b"SNSS" {
            return None;
        }
        // 1 · 3: 그대로 적은 파일, 2 · 4: 암호화된 파일
        let version = i32::from_le_bytes(data[4..8].try_into().ok()?);
        if version != 1 && version != 3 {
            return None;
        }
        let mut session = Self::default();
        let mut position = 8;
        while position + 2 <= data.len() {
            let size = usize::from(u16::from_le_bytes([data[position], data[position + 1]]));
            position += 2;
            // 쓰는 도중에 읽은 마지막 명령은 건너뛴다.
            if size == 0 || position + size > data.len() {
                break;
            }
            session.apply(data[position], &data[position + 1..position + size]);
            position += size;
        }
        Some(session)
    }

    fn tab(&mut self, id: i32) -> &mut TabState {
        self.tabs.entry(id).or_default()
    }

    fn note_window(&mut self, id: i32) {
        if !self.window_order.contains(&id) {
            self.window_order.push(id);
        }
    }

    fn apply(&mut self, id: u8, payload: &[u8]) {
        let int = |index: usize| payload.get(index * 4..index * 4 + 4).map(|bytes| i32::from_le_bytes(bytes.try_into().expect("4 바이트")));
        match id {
            command::SET_TAB_WINDOW => {
                if let (Some(window), Some(tab)) = (int(0), int(1)) {
                    self.tab(tab).window = Some(window);
                    self.note_window(window);
                }
            }
            command::SET_TAB_INDEX_IN_WINDOW => {
                if let (Some(tab), Some(index)) = (int(0), int(1)) {
                    self.tab(tab).index = index;
                }
            }
            command::UPDATE_TAB_NAVIGATION => {
                let Some(mut pickle) = Pickle::new(payload) else {
                    return;
                };
                if let (Some(tab), Some(index), Some(url), Some(title)) = (pickle.int(), pickle.int(), pickle.string(), pickle.string16()) {
                    self.tab(tab).navigations.insert(index, (url, title));
                }
            }
            command::SET_SELECTED_NAVIGATION_INDEX => {
                if let (Some(tab), Some(index)) = (int(0), int(1)) {
                    self.tab(tab).selected = Some(index);
                }
            }
            command::SET_WINDOW_TYPE => {
                if let (Some(window), Some(kind)) = (int(0), int(1)) {
                    self.window_types.insert(window, kind);
                    self.note_window(window);
                }
            }
            command::PRUNED_FROM_BACK => {
                // 이 번호부터 뒤의 기록을 지운다.
                if let (Some(tab), Some(index)) = (int(0), int(1)) {
                    let tab = self.tab(tab);
                    tab.navigations.retain(|number, _| *number < index);
                    tab.selected = tab.selected.map(|selected| selected.min(index - 1).max(0));
                }
            }
            command::PRUNED_FROM_FRONT => {
                // 앞에서 count 개를 지우고 번호를 당긴다.
                if let (Some(tab), Some(count)) = (int(0), int(1)) {
                    Self::prune(self.tab(tab), 0, count);
                }
            }
            command::NAVIGATION_PATH_PRUNED => {
                if let (Some(tab), Some(index), Some(count)) = (int(0), int(1), int(2)) {
                    Self::prune(self.tab(tab), index, count);
                }
            }
            command::TAB_CLOSED => {
                if let Some(tab) = int(0) {
                    self.tabs.remove(&tab);
                }
            }
            command::WINDOW_CLOSED => {
                if let Some(window) = int(0) {
                    self.tabs.retain(|_, tab| tab.window != Some(window));
                    self.window_types.remove(&window);
                    self.window_order.retain(|other| *other != window);
                }
            }
            command::SET_ACTIVE_WINDOW => {
                self.active_window = int(0);
            }
            command::SET_TAB_GROUP => {
                // { int32 탭, (4바이트 빈칸), uint64 위, uint64 아래, bool 그룹 있음 }
                let Some(tab) = int(0) else {
                    return;
                };
                let (Some(high), Some(low), Some(&has_group)) = (read_u64(payload, 8), read_u64(payload, 16), payload.get(24)) else {
                    return;
                };
                self.tab(tab).group = (has_group != 0).then_some((u128::from(high) << 64) | u128::from(low));
            }
            command::SET_TAB_GROUP_METADATA => {
                let Some(mut pickle) = Pickle::new(payload) else {
                    return;
                };
                if let (Some(high), Some(low), Some(title), Some(color)) = (pickle.u64(), pickle.u64(), pickle.string16(), pickle.u32()) {
                    self.groups.insert((u128::from(high) << 64) | u128::from(low), (title, color));
                }
            }
            _ => {}
        }
    }

    /// 탐색 기록에서 [index, index + count) 를 지우고 뒤 번호를 count 만큼 당긴다.
    fn prune(tab: &mut TabState, index: i32, count: i32) {
        if count <= 0 {
            return;
        }
        let navigations = std::mem::take(&mut tab.navigations);
        tab.navigations = navigations
            .into_iter()
            .filter_map(|(number, entry)| {
                if number < index {
                    Some((number, entry))
                } else if number >= index + count {
                    Some((number - count, entry))
                } else {
                    None
                }
            })
            .collect();
        tab.selected = tab.selected.map(|selected| {
            if selected >= index + count {
                selected - count
            } else if selected >= index {
                (index - 1).max(0)
            } else {
                selected
            }
        });
    }

    /// 열린 일반 창마다 (창 번호, 탭(순서대로)). 탭이 없는 창은 뺀다.
    fn windows(&self) -> Vec<(i32, Vec<&TabState>)> {
        self.window_order
            .iter()
            .filter(|window| self.window_types.get(window).copied().unwrap_or(NORMAL_WINDOW) == NORMAL_WINDOW)
            .filter_map(|window| {
                let mut tabs: Vec<(&i32, &TabState)> = self.tabs.iter().filter(|(_, tab)| tab.window == Some(*window) && !tab.navigations.is_empty()).collect();
                tabs.sort_by_key(|(id, tab)| (tab.index, **id));
                let tabs: Vec<&TabState> = tabs.into_iter().map(|(_, tab)| tab).collect();
                (!tabs.is_empty()).then_some((*window, tabs))
            })
            .collect()
    }
}

fn read_u64(data: &[u8], at: usize) -> Option<u64> {
    data.get(at..at + 8).map(|bytes| u64::from_le_bytes(bytes.try_into().expect("8 바이트")))
}

/// base::Pickle 읽기: 앞 4바이트는 내용 길이, 값은 4바이트 단위로 맞춰 적혀 있다.
struct Pickle<'a> {
    data: &'a [u8],
    position: usize,
}

impl<'a> Pickle<'a> {
    fn new(payload: &'a [u8]) -> Option<Self> {
        let size = usize::try_from(u32::from_le_bytes(payload.get(..4)?.try_into().ok()?)).ok()?;
        Some(Self { data: payload.get(4..4 + size)?, position: 0 })
    }

    fn bytes(&mut self, length: usize) -> Option<&'a [u8]> {
        let value = self.data.get(self.position..self.position.checked_add(length)?)?;
        self.position += length.div_ceil(4) * 4;
        Some(value)
    }

    fn int(&mut self) -> Option<i32> {
        Some(i32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    fn u32(&mut self) -> Option<u32> {
        Some(u32::from_le_bytes(self.bytes(4)?.try_into().ok()?))
    }

    fn u64(&mut self) -> Option<u64> {
        Some(u64::from_le_bytes(self.bytes(8)?.try_into().ok()?))
    }

    fn string(&mut self) -> Option<String> {
        let length = usize::try_from(self.int()?).ok()?;
        Some(String::from_utf8_lossy(self.bytes(length)?).into_owned())
    }

    fn string16(&mut self) -> Option<String> {
        let length = usize::try_from(self.int()?).ok()?;
        let units: Vec<u16> = self.bytes(length.checked_mul(2)?)?.as_chunks::<2>().0.iter().map(|pair| u16::from_le_bytes(*pair)).collect();
        Some(String::from_utf16_lossy(&units))
    }
}

/// 마커 탭(앱이 확장과 주고받으려고 잠깐 여는 탭)인지
fn is_marker(url: &str) -> bool {
    url.starts_with(&format!("https://{}/", brand::MARKER_HOST))
}

/// 여러 프로필의 세션을 확장 스냅샷과 같은 모양으로 합친다. sessions[0] 의 활성 창이 맨 앞 창이다.
pub fn snapshot_of(sessions: &[Session], browser: Browser, saved_at: Option<Timestamp>) -> ChromeSnapshot {
    let mut windows = Vec::new();
    let mut groups: Vec<SnapshotTabGroup> = Vec::new();
    let mut group_ids: HashMap<(usize, u128), i64> = HashMap::new();
    for (session_index, session) in sessions.iter().enumerate() {
        for (window, tabs) in session.windows() {
            let window_id = windows.len() as i64 + 1;
            let mut snapshot_tabs = Vec::new();
            for tab in tabs {
                let Some((url, title)) = tab.selected.and_then(|selected| tab.navigations.get(&selected)).or_else(|| tab.navigations.values().last()) else {
                    continue;
                };
                if is_marker(url) {
                    continue;
                }
                let group_id = tab.group.map_or(-1, |token| {
                    *group_ids.entry((session_index, token)).or_insert_with(|| {
                        let id = groups.len() as i64 + 1;
                        let (title, color) = session.groups.get(&token).cloned().unwrap_or_default();
                        let color = COLORS.get(color as usize).copied().unwrap_or("grey").to_owned();
                        groups.push(SnapshotTabGroup { id, title, color, window_id });
                        id
                    })
                });
                snapshot_tabs.push(SnapshotTab { title: title.clone(), url: url.clone(), group_id });
            }
            if !snapshot_tabs.is_empty() {
                windows.push(SnapshotWindow { id: window_id, focused: session_index == 0 && session.active_window == Some(window), tabs: snapshot_tabs });
            }
        }
    }
    ChromeSnapshot {
        nonce: None,
        saved_at: saved_at.map(|time| time.unix_seconds() * 1000.0),
        browser: Some(browser.raw_value().to_owned()),
        windows,
        groups,
    }
}

pub struct SessionFileReader;

impl SessionFileReader {
    /// 브라우저의 지금 열린 창 · 탭 · 탭 그룹. 브라우저가 이번에 켜진 뒤 쓴 세션 파일만 읽는다
    /// (열려 있지 않은 프로필의 지난 세션은 뺀다). 읽을 수 있는 파일이 없거나 열린 창이 없으면 None.
    pub fn read(browser: Browser) -> Option<ChromeSnapshot> {
        let root = browser.user_data_directory();
        let launched = browser.launch_date();
        let mut files: Vec<(Timestamp, PathBuf)> = ChromeExtensionDetector::profile_names(&root)
            .into_iter()
            .filter_map(|profile| Self::latest_session_file(&root.join(profile).join("Sessions")))
            .collect();
        // 최근에 쓴 것이 먼저 (그 프로필의 활성 창이 맨 앞 창)
        files.sort_by(|a, b| b.0.partial_cmp(&a.0).unwrap_or(std::cmp::Ordering::Equal));
        match launched {
            Some(launched) => files.retain(|(modified, _)| *modified >= launched),
            // 켜진 시각을 모르면 가장 최근 것 하나만
            None => files.truncate(1),
        }
        let sessions: Vec<Session> = files.iter().filter_map(|(_, path)| Session::parse(&fs::read(path).ok()?)).collect();
        let snapshot = snapshot_of(&sessions, browser, files.first().map(|(modified, _)| *modified));
        (!snapshot.windows.is_empty()).then_some(snapshot)
    }

    /// 폴더에서 가장 최근에 쓴 `Session_*` 파일
    fn latest_session_file(folder: &Path) -> Option<(Timestamp, PathBuf)> {
        fs::read_dir(folder)
            .ok()?
            .filter_map(|entry| entry.ok())
            .filter(|entry| entry.file_name().to_string_lossy().starts_with("Session_"))
            .filter_map(|entry| Some((Timestamp::from(entry.metadata().ok()?.modified().ok()?), entry.path())))
            .max_by(|a, b| a.0.partial_cmp(&b.0).unwrap_or(std::cmp::Ordering::Equal))
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    /// 시험용 세션 파일을 만든다 (Chromium 이 쓰는 것과 같은 모양).
    struct Writer(Vec<u8>);

    impl Writer {
        fn new() -> Self {
            let mut data = b"SNSS".to_vec();
            data.extend(3i32.to_le_bytes());
            Self(data)
        }

        fn command(&mut self, id: u8, payload: &[u8]) -> &mut Self {
            self.0.extend(u16::try_from(payload.len() + 1).unwrap().to_le_bytes());
            self.0.push(id);
            self.0.extend(payload);
            self
        }

        fn ints(&mut self, id: u8, values: &[i32]) -> &mut Self {
            let payload: Vec<u8> = values.iter().flat_map(|value| value.to_le_bytes()).collect();
            self.command(id, &payload)
        }

        fn pickle(&mut self, id: u8, write: impl FnOnce(&mut Vec<u8>)) -> &mut Self {
            let mut body = Vec::new();
            write(&mut body);
            let mut payload = u32::try_from(body.len()).unwrap().to_le_bytes().to_vec();
            payload.extend(body);
            self.command(id, &payload)
        }

        fn navigation(&mut self, tab: i32, index: i32, url: &str, title: &str) -> &mut Self {
            self.pickle(command::UPDATE_TAB_NAVIGATION, |body| {
                body.extend(tab.to_le_bytes());
                body.extend(index.to_le_bytes());
                put_string(body, url.as_bytes());
                put_string16(body, title);
                body.extend(0i32.to_le_bytes()); // 그 뒤 값들(쓰지 않음)
            })
        }

        fn group(&mut self, tab: i32, token: Option<(u64, u64)>) -> &mut Self {
            let mut payload = tab.to_le_bytes().to_vec();
            payload.extend([0; 4]);
            let (high, low) = token.unwrap_or_default();
            payload.extend(high.to_le_bytes());
            payload.extend(low.to_le_bytes());
            payload.push(u8::from(token.is_some()));
            payload.extend([0; 7]);
            self.command(command::SET_TAB_GROUP, &payload)
        }

        fn group_metadata(&mut self, (high, low): (u64, u64), title: &str, color: u32) -> &mut Self {
            self.pickle(command::SET_TAB_GROUP_METADATA, |body| {
                body.extend(high.to_le_bytes());
                body.extend(low.to_le_bytes());
                put_string16(body, title);
                body.extend(color.to_le_bytes());
                body.extend(1i32.to_le_bytes()); // 접힘
            })
        }
    }

    fn pad(body: &mut Vec<u8>) {
        while !body.len().is_multiple_of(4) {
            body.push(0);
        }
    }

    fn put_string(body: &mut Vec<u8>, bytes: &[u8]) {
        body.extend(i32::try_from(bytes.len()).unwrap().to_le_bytes());
        body.extend(bytes);
        pad(body);
    }

    fn put_string16(body: &mut Vec<u8>, text: &str) {
        let units: Vec<u16> = text.encode_utf16().collect();
        body.extend(i32::try_from(units.len()).unwrap().to_le_bytes());
        body.extend(units.iter().flat_map(|unit| unit.to_le_bytes()));
        pad(body);
    }

    const WORK: (u64, u64) = (0x1111, 0x2222);

    fn sample() -> Vec<u8> {
        let mut file = Writer::new();
        file.ints(command::SET_WINDOW_TYPE, &[1, 0])
            .ints(command::SET_TAB_WINDOW, &[1, 10])
            .ints(command::SET_TAB_INDEX_IN_WINDOW, &[10, 1])
            .navigation(10, 0, "https://a.example/old", "예전")
            .navigation(10, 1, "https://a.example/now", "지금 A")
            .ints(command::SET_SELECTED_NAVIGATION_INDEX, &[10, 1])
            .ints(command::SET_TAB_WINDOW, &[1, 11])
            .ints(command::SET_TAB_INDEX_IN_WINDOW, &[11, 0])
            .navigation(11, 0, "https://b.example/", "B 문서")
            .group(11, Some(WORK))
            .group(10, Some(WORK))
            .group_metadata(WORK, "업무 🗂", 4)
            // 닫은 탭 · 마커 탭 · 그룹에서 뺀 탭
            .ints(command::SET_TAB_WINDOW, &[1, 12])
            .navigation(12, 0, "https://closed.example/", "닫음")
            .ints(command::TAB_CLOSED, &[12, 0, 0])
            .ints(command::SET_TAB_WINDOW, &[1, 13])
            .ints(command::SET_TAB_INDEX_IN_WINDOW, &[13, 2])
            .navigation(13, 0, "https://linkgroupz.invalid/snapshot#x", "")
            .ints(command::SET_TAB_WINDOW, &[1, 14])
            .ints(command::SET_TAB_INDEX_IN_WINDOW, &[14, 3])
            .navigation(14, 0, "https://c.example/", "C")
            .group(14, Some(WORK))
            .group(14, None)
            // 팝업 창은 뺀다
            .ints(command::SET_WINDOW_TYPE, &[2, 1])
            .ints(command::SET_TAB_WINDOW, &[2, 20])
            .navigation(20, 0, "https://popup.example/", "팝업")
            .ints(command::SET_ACTIVE_WINDOW, &[1]);
        // 쓰는 도중에 끊긴 마지막 명령
        file.0.extend([40, 0, 6, 1, 2]);
        file.0
    }

    #[test]
    fn reads_windows_tabs_and_groups() {
        let session = Session::parse(&sample()).unwrap();
        let snapshot = snapshot_of(&[session], Browser::Whale, None);
        assert_eq!(snapshot.browser.as_deref(), Some("whale"));
        assert_eq!(snapshot.windows.len(), 1, "팝업 창은 빼고 일반 창만");
        let window = &snapshot.windows[0];
        assert!(window.focused);
        let tabs: Vec<(&str, &str, i64)> = window.tabs.iter().map(|tab| (tab.url.as_str(), tab.title.as_str(), tab.group_id)).collect();
        assert_eq!(
            tabs,
            [("https://b.example/", "B 문서", 1), ("https://a.example/now", "지금 A", 1), ("https://c.example/", "C", -1)],
            "탭 순서대로, 지금 보고 있는 기록, 닫은 탭 · 마커 탭은 빼고, 그룹에서 뺀 탭은 그룹 없음"
        );
        assert_eq!(snapshot.groups.len(), 1);
        assert_eq!((snapshot.groups[0].title.as_str(), snapshot.groups[0].color.as_str()), ("업무 🗂", "green"));
        // 앱이 쓰는 탭 그룹 목록으로도 바뀐다
        let imports = snapshot.tab_groups();
        assert_eq!(imports.len(), 1);
    }

    #[test]
    fn rejects_other_files() {
        assert!(Session::parse(b"").is_none());
        assert!(Session::parse(b"SNSS\x04\0\0\0").is_none(), "암호화된 세션 파일은 읽지 않는다");
        assert!(Session::parse(b"NOPE\x03\0\0\0").is_none());
        let empty = Session::parse(b"SNSS\x03\0\0\0").unwrap();
        assert!(snapshot_of(&[empty], Browser::Chrome, None).windows.is_empty());
    }

    #[test]
    fn closed_window_and_pruned_history() {
        let mut file = Writer::new();
        file.ints(command::SET_TAB_WINDOW, &[1, 10])
            .navigation(10, 0, "https://x.example/0", "0")
            .navigation(10, 1, "https://x.example/1", "1")
            .navigation(10, 2, "https://x.example/2", "2")
            .ints(command::SET_SELECTED_NAVIGATION_INDEX, &[10, 2])
            .ints(command::NAVIGATION_PATH_PRUNED, &[10, 0, 1])
            .ints(command::SET_TAB_WINDOW, &[2, 20])
            .navigation(20, 0, "https://y.example/", "y")
            .ints(command::WINDOW_CLOSED, &[2, 0, 0]);
        let snapshot = snapshot_of(&[Session::parse(&file.0).unwrap()], Browser::Chrome, None);
        assert_eq!(snapshot.windows.len(), 1, "닫은 창은 뺀다");
        assert_eq!(snapshot.windows[0].tabs[0].url, "https://x.example/2", "기록을 지운 뒤에도 보고 있던 주소");
    }
}
