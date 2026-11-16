//! 브라우저에 저장된(닫아 둔) 탭 그룹을 읽는다.
//!
//! Chrome 계열은 탭 그룹을 닫아도 '저장된 탭 그룹'으로 남겨 두고, 프로필의 `Sync Data/LevelDB` 에
//! `saved_tab_group-dt-<guid>` 키로 적는다 (동기화를 켜지 않아도 적힌다). 값은 protobuf 이다:
//!
//! ```text
//! SavedTabGroupData { 2: SavedTabGroupSpecifics {
//!     1: guid, 3: 고친 시각(1601년부터 µs),
//!     4: 그룹 { 2: 이름, 3: 색 }  또는  5: 탭 { 1: 그룹 guid, 2: 순서, 3: URL, 4: 제목 } } }
//! ```
//!
//! 열려 있는 탭 그룹도 함께 저장돼 있으므로, 지금 열린 그룹(이름 · 색이 같은 것)은 빼고 남은 것을 '닫힌 탭 그룹'으로 본다.

use crate::browser::Browser;
use crate::leveldb::{read_directory, Reader};
use crate::models::GroupColor;
use crate::snapshot::ChromeTabGroupImport;
use crate::tabs::{ChromeTab, ChromeTabGroupInfo};
use crate::timestamp::Timestamp;
use std::collections::HashMap;
use std::path::Path;

const KEY_PREFIX: &[u8] = b"saved_tab_group-dt-";

/// 저장된 탭 그룹 하나
#[derive(Clone, Debug, PartialEq)]
pub struct SavedTabGroup {
    pub guid: String,
    pub title: String,
    pub color: GroupColor,
    pub updated_at: Option<Timestamp>,
    pub tabs: Vec<SavedTab>,
}

#[derive(Clone, Debug, PartialEq, Eq)]
pub struct SavedTab {
    pub position: i64,
    pub url: String,
    pub title: String,
}

/// protobuf 필드 하나 (필요한 형식만)
enum Field<'a> {
    Varint(u64),
    Bytes(&'a [u8]),
}

/// 메시지의 (필드 번호, 값)들. 읽지 못하면 그때까지 읽은 것만 돌려준다.
fn fields(data: &[u8]) -> Vec<(u64, Field<'_>)> {
    let mut reader = Reader::new(data);
    let mut result = Vec::new();
    while !reader.is_empty() {
        let Some(tag) = reader.varint() else {
            break;
        };
        let value = match tag & 7 {
            0 => reader.varint().map(Field::Varint),
            1 => reader.bytes(8).map(|_| Field::Varint(0)),
            5 => reader.bytes(4).map(|_| Field::Varint(0)),
            2 => reader.varint().and_then(|length| reader.bytes(usize::try_from(length).ok()?)).map(Field::Bytes),
            _ => None,
        };
        let Some(value) = value else {
            break;
        };
        result.push((tag >> 3, value));
    }
    result
}

fn bytes_field<'a>(fields: &[(u64, Field<'a>)], number: u64) -> Option<&'a [u8]> {
    fields.iter().find_map(|(field, value)| match value {
        Field::Bytes(bytes) if *field == number => Some(*bytes),
        _ => None,
    })
}

fn text_field(fields: &[(u64, Field<'_>)], number: u64) -> Option<String> {
    bytes_field(fields, number).map(|bytes| String::from_utf8_lossy(bytes).into_owned())
}

fn varint_field(fields: &[(u64, Field<'_>)], number: u64) -> Option<u64> {
    fields.iter().find_map(|(field, value)| match value {
        Field::Varint(value) if *field == number => Some(*value),
        _ => None,
    })
}

/// 동기화 데이터의 탭 그룹 색 번호 (SavedTabGroupColor)
fn color(number: u64) -> GroupColor {
    match number {
        2 => GroupColor::Blue,
        3 => GroupColor::Red,
        4 => GroupColor::Yellow,
        5 => GroupColor::Green,
        6 => GroupColor::Pink,
        7 => GroupColor::Purple,
        8 => GroupColor::Cyan,
        9 => GroupColor::Orange,
        _ => GroupColor::Grey,
    }
}

/// 1601-01-01 부터의 µs → 유닉스 초
fn timestamp(windows_micros: u64) -> Option<Timestamp> {
    const EPOCH_DIFFERENCE_SECONDS: f64 = 11_644_473_600.0;
    (windows_micros > 0).then(|| Timestamp::from_unix_seconds(windows_micros as f64 / 1_000_000.0 - EPOCH_DIFFERENCE_SECONDS))
}

enum Entity {
    Group { guid: String, title: String, color: GroupColor, updated_at: Option<Timestamp> },
    Tab { group_guid: String, tab: SavedTab },
}

fn parse_record(value: &[u8]) -> Option<Entity> {
    let outer = fields(value);
    // SavedTabGroupData 로 감싸 두지 않은 예전 기록이면 값 자체가 SavedTabGroupSpecifics 다.
    let specifics_bytes = bytes_field(&outer, 2).filter(|bytes| bytes_field(&fields(bytes), 1).is_some()).unwrap_or(value);
    let specifics = fields(specifics_bytes);
    let guid = text_field(&specifics, 1)?;
    if let Some(group) = bytes_field(&specifics, 4) {
        let group = fields(group);
        return Some(Entity::Group {
            guid,
            title: text_field(&group, 2).unwrap_or_default(),
            color: color(varint_field(&group, 3).unwrap_or(0)),
            updated_at: varint_field(&specifics, 3).and_then(timestamp),
        });
    }
    let tab = fields(bytes_field(&specifics, 5)?);
    Some(Entity::Tab {
        group_guid: text_field(&tab, 1)?,
        tab: SavedTab {
            position: varint_field(&tab, 2).map_or(0, |value| value as i64),
            url: text_field(&tab, 3)?,
            title: text_field(&tab, 4).unwrap_or_default(),
        },
    })
}

/// 동기화 저장소 폴더 하나의 저장된 탭 그룹 (탭이 있는 것만, 최근에 고친 것이 앞)
pub fn read_store(directory: &Path) -> Vec<SavedTabGroup> {
    let Ok(records) = read_directory(directory, KEY_PREFIX) else {
        return Vec::new();
    };
    let mut groups: Vec<SavedTabGroup> = Vec::new();
    let mut tabs: HashMap<String, Vec<SavedTab>> = HashMap::new();
    for value in records.values() {
        match parse_record(value) {
            Some(Entity::Group { guid, title, color, updated_at }) => {
                groups.push(SavedTabGroup { guid, title, color, updated_at, tabs: Vec::new() })
            }
            Some(Entity::Tab { group_guid, tab }) => tabs.entry(group_guid).or_default().push(tab),
            None => {}
        }
    }
    for group in &mut groups {
        let mut members = tabs.remove(&group.guid).unwrap_or_default();
        members.sort_by_key(|tab| tab.position);
        group.tabs = members;
    }
    groups.retain(|group| !group.tabs.is_empty());
    groups.sort_by(|a, b| {
        let key = |group: &SavedTabGroup| group.updated_at.map_or(f64::MIN, |time| time.unix_seconds());
        key(b).total_cmp(&key(a))
    });
    groups
}

/// 브라우저의 모든 프로필에 저장된 탭 그룹 (같은 그룹은 한 번만)
pub fn read_saved_tab_groups(browser: Browser) -> Vec<SavedTabGroup> {
    let Ok(entries) = std::fs::read_dir(browser.user_data_directory()) else {
        return Vec::new();
    };
    let mut profiles: Vec<_> = entries
        .filter_map(|entry| entry.ok())
        .map(|entry| entry.path())
        .filter(|path| {
            let name = path.file_name().map(|name| name.to_string_lossy().into_owned()).unwrap_or_default();
            name != "Guest Profile" && name != "System Profile"
        })
        .map(|path| path.join("Sync Data").join("LevelDB"))
        .filter(|path| path.is_dir())
        .collect();
    profiles.sort();
    let mut seen = std::collections::HashSet::new();
    profiles.iter().flat_map(|directory| read_store(directory)).filter(|group| seen.insert(group.guid.clone())).collect()
}

/// 저장된 탭 그룹 가운데 지금 열려 있지 않은 것을 가져오기 목록 모양으로.
/// 열린 그룹과 이름 · 색이 같으면 열린 것으로 본다 (같은 이름 · 색이 여럿이면 그 수만큼).
pub fn closed_tab_groups(saved: &[SavedTabGroup], open: &[ChromeTabGroupImport]) -> Vec<ChromeTabGroupImport> {
    let key = |title: &str, color: GroupColor| (title.trim().to_lowercase(), color);
    let mut open_counts: HashMap<(String, GroupColor), usize> = HashMap::new();
    for group in open {
        *open_counts.entry(key(&group.info.title, group.info.color)).or_default() += 1;
    }
    saved
        .iter()
        .filter(|group| match open_counts.get_mut(&key(&group.title, group.color)) {
            Some(count) if *count > 0 => {
                *count -= 1;
                false
            }
            _ => true,
        })
        .enumerate()
        .map(|(index, group)| {
            // 열린 탭 그룹의 번호(양수)와 겹치지 않게 음수 번호를 붙인다.
            let mut info = ChromeTabGroupInfo::new(-(index as i64) - 1, group.title.clone(), group.color);
            info.is_closed = true;
            let tabs = group
                .tabs
                .iter()
                .map(|tab| ChromeTab { tab_group: Some(info.clone()), ..ChromeTab::new(0, tab.title.clone(), tab.url.clone()) })
                .collect();
            ChromeTabGroupImport { info, window_index: 0, tabs }
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn varint(mut value: u64, out: &mut Vec<u8>) {
        loop {
            let byte = (value & 0x7f) as u8;
            value >>= 7;
            if value == 0 {
                out.push(byte);
                return;
            }
            out.push(byte | 0x80);
        }
    }

    fn bytes(number: u64, data: &[u8], out: &mut Vec<u8>) {
        varint(number << 3 | 2, out);
        varint(data.len() as u64, out);
        out.extend_from_slice(data);
    }

    fn number(number: u64, value: u64, out: &mut Vec<u8>) {
        varint(number << 3, out);
        varint(value, out);
    }

    fn record(guid: &str, entity: u64, body: &[u8]) -> Vec<u8> {
        let mut specifics = Vec::new();
        bytes(1, guid.as_bytes(), &mut specifics);
        number(3, 13_400_000_000_000_000, &mut specifics);
        bytes(entity, body, &mut specifics);
        let mut data = Vec::new();
        number(1, 1, &mut data);
        bytes(2, &specifics, &mut data);
        data
    }

    #[test]
    fn records() {
        let mut group = Vec::new();
        bytes(2, "업무".as_bytes(), &mut group);
        number(3, 5, &mut group);
        let Some(Entity::Group { guid, title, color, updated_at }) = parse_record(&record("g1", 4, &group)) else {
            panic!("그룹");
        };
        assert_eq!((guid.as_str(), title.as_str(), color), ("g1", "업무", GroupColor::Green));
        assert!(updated_at.is_some_and(|time| time.unix_seconds() > 1_700_000_000.0), "1601년 기준 µs → 유닉스 초");

        let mut tab = Vec::new();
        bytes(1, b"g1", &mut tab);
        number(2, 3, &mut tab);
        bytes(3, b"https://a.com/", &mut tab);
        bytes(4, "제목".as_bytes(), &mut tab);
        let Some(Entity::Tab { group_guid, tab }) = parse_record(&record("t1", 5, &tab)) else {
            panic!("탭");
        };
        assert_eq!((group_guid.as_str(), tab.position, tab.url.as_str(), tab.title.as_str()), ("g1", 3, "https://a.com/", "제목"));
        assert!(parse_record(b"\xff\xff").is_none(), "깨진 값");
    }

    #[test]
    fn closed_groups_exclude_open_ones() {
        let saved = |title: &str, color: GroupColor| SavedTabGroup {
            guid: title.to_owned(),
            title: title.to_owned(),
            color,
            updated_at: None,
            tabs: vec![SavedTab { position: 0, url: "https://a.com".to_owned(), title: String::new() }],
        };
        let open = |title: &str, color: GroupColor| ChromeTabGroupImport {
            info: ChromeTabGroupInfo::new(7, title, color),
            window_index: 0,
            tabs: Vec::new(),
        };
        let all = [saved("A", GroupColor::Blue), saved("A", GroupColor::Red), saved("B", GroupColor::Blue)];
        let closed = closed_tab_groups(&all, &[open(" a ", GroupColor::Blue)]);
        let names: Vec<(String, GroupColor, bool)> =
            closed.iter().map(|group| (group.info.title.clone(), group.info.color, group.info.is_closed)).collect();
        assert_eq!(names, [("A".to_owned(), GroupColor::Red, true), ("B".to_owned(), GroupColor::Blue, true)], "이름 · 색이 같은 열린 그룹은 뺀다");
        assert!(closed.iter().all(|group| group.id() < 0), "열린 그룹 번호와 겹치지 않는다");
        assert_eq!(closed_tab_groups(&all, &[]).len(), 3, "브라우저가 꺼져 있으면 모두 닫힌 그룹");
    }
}
