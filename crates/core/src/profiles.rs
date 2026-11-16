//! 프로필: 그룹 목록을 따로 두는 묶음 (예: 회사 · 개인).
//!
//! 프로필마다 data.json 이 따로 있고, 관리 창 · 메뉴바 창 · 검색은 지금 프로필의 그룹만 다룬다.
//! 처음부터 있던 '기본' 프로필은 지금까지의 data.json 을 그대로 쓰고(원본 LinkGroups 와 같은 자리),
//! 더한 프로필은 데이터 폴더의 `profiles/<id>/data.json` 에 둔다. 백업도 프로필 폴더마다 따로 둔다.
//!
//! 프로필을 지우면 그 폴더를 지우지 않고 기본 백업 폴더의 `deleted-profiles/` 로 옮겨 둔다
//! (거기 든 data.json 은 설정 → 가져오기로 되살릴 수 있다).

use crate::backups::Backups;
use crate::group_icon::GroupIcon;
use crate::ids::Id;
use crate::models::GroupColor;
use serde::{Deserialize, Serialize};
use std::fs;
use std::io;
use std::path::{Path, PathBuf};

pub const DEFAULT_PROFILE_ID: &str = "default";
pub const DEFAULT_PROFILE_NAME: &str = "기본";
/// 프로필 이름 최대 글자 수
pub const NAME_LIMIT: usize = 30;

const FOLDER: &str = "profiles";
const DELETED_FOLDER: &str = "deleted-profiles";

#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Profile {
    /// "default" 또는 UUID (폴더 이름으로 쓴다)
    pub id: String,
    pub name: String,
    /// 프로필 버튼 · 설정에 보이는 아이콘 (그룹 아이콘과 같은 기본 모양 또는 내 이미지)
    #[serde(default = "Profile::default_icon")]
    pub icon: GroupIcon,
    /// 아이콘 바탕색
    #[serde(default = "Profile::default_color")]
    pub color: GroupColor,
}

impl Profile {
    /// 화면에 보일 이름. 기본 프로필의 이름을 바꾸지 않았으면 지금 언어의 '기본'.
    pub fn display_name(&self) -> String {
        if self.is_default() && (self.name == DEFAULT_PROFILE_NAME || self.name == "Default") {
            crate::tr!("기본", "Default").to_owned()
        } else {
            self.name.clone()
        }
    }

    pub fn default_icon() -> GroupIcon {
        GroupIcon::Symbol("person.2.fill".to_owned())
    }

    pub fn default_color() -> GroupColor {
        GroupColor::Blue
    }

    pub fn default_profile() -> Self {
        Self { id: DEFAULT_PROFILE_ID.to_owned(), name: DEFAULT_PROFILE_NAME.to_owned(), icon: Self::default_icon(), color: Self::default_color() }
    }

    /// 새 프로필. 이름이 비었거나 너무 길면 None
    pub fn new(name: &str) -> Option<Self> {
        Some(Self { id: Id::new().to_string(), name: clean_name(name)?, icon: Self::default_icon(), color: Self::default_color() })
    }

    pub fn is_default(&self) -> bool {
        self.id == DEFAULT_PROFILE_ID
    }

    /// 폴더 이름으로 써도 되는 id 인지 ("default" 또는 UUID). 설정 파일을 고쳐 `../` 같은 값을 넣어도 데이터 폴더 밖을 가리키지 않게 한다.
    fn has_valid_id(&self) -> bool {
        self.is_default() || Id::parse(&self.id).is_some_and(|id| id.to_string() == self.id)
    }

    /// 이 프로필의 폴더 (기본 프로필은 앱 데이터 폴더 그대로)
    pub fn directory(&self, data_directory: &Path) -> PathBuf {
        if self.is_default() {
            data_directory.to_path_buf()
        } else {
            data_directory.join(FOLDER).join(&self.id)
        }
    }

    pub fn data_file(&self, data_directory: &Path) -> PathBuf {
        self.directory(data_directory).join("data.json")
    }

    pub fn backup_directory(&self, data_directory: &Path) -> PathBuf {
        Backups::directory(&self.directory(data_directory))
    }

    /// 지운 프로필의 폴더를 기본 백업 폴더의 `deleted-profiles/<시각> <이름>` 으로 옮긴다.
    /// 데이터가 한 번도 저장되지 않아 폴더가 없으면 아무것도 하지 않고 None.
    pub fn retire(&self, data_directory: &Path, now_stamp: &str) -> io::Result<Option<PathBuf>> {
        if self.is_default() {
            return Err(io::Error::new(io::ErrorKind::InvalidInput, crate::tr!("기본 프로필은 지울 수 없습니다", "The default profile can't be deleted")));
        }
        let folder = self.directory(data_directory);
        if !folder.is_dir() {
            return Ok(None);
        }
        let parent = Backups::directory(data_directory).join(DELETED_FOLDER);
        fs::create_dir_all(&parent)?;
        let mut target = parent.join(format!("{now_stamp} {}", folder_safe(&self.name)));
        if target.exists() {
            target = parent.join(format!("{now_stamp} {} {}", folder_safe(&self.name), self.id));
        }
        fs::rename(&folder, &target)?;
        Ok(Some(target))
    }
}

/// 앞뒤 공백을 지운 이름. 비었거나 NAME_LIMIT 글자를 넘으면 None
pub fn clean_name(name: &str) -> Option<String> {
    let name = name.split_whitespace().collect::<Vec<_>>().join(" ");
    (!name.is_empty() && name.chars().count() <= NAME_LIMIT).then_some(name)
}

/// 설정에 적힌 목록을 쓸 수 있게 고친다: 기본 프로필이 맨 앞에 꼭 하나 있고,
/// id 가 겹치거나 폴더 이름으로 쓸 수 없는 것은 빼고, 이름이 비면 기본 이름을 붙인다.
pub fn normalized(list: &[Profile]) -> Vec<Profile> {
    let mut result = vec![list
        .iter()
        .find(|profile| profile.is_default())
        .map(|profile| Profile { name: clean_name(&profile.name).unwrap_or_else(|| DEFAULT_PROFILE_NAME.to_owned()), ..profile.clone() })
        .unwrap_or_else(Profile::default_profile)];
    for profile in list {
        if profile.is_default() || !profile.has_valid_id() || result.iter().any(|kept| kept.id == profile.id) {
            continue;
        }
        let name = clean_name(&profile.name).unwrap_or_else(|| crate::tr!("이름 없는 프로필", "Untitled profile").to_owned());
        result.push(Profile { name, ..profile.clone() });
    }
    result
}

/// 목록에서 id 로 찾는다. 없으면 기본 프로필
pub fn find(list: &[Profile], id: &str) -> Profile {
    let list = normalized(list);
    list.iter().find(|profile| profile.id == id).cloned().unwrap_or_else(|| list[0].clone())
}

/// 폴더 이름에 쓸 수 없는 글자를 바꾼다.
fn folder_safe(name: &str) -> String {
    name.chars().map(|character| if character.is_control() || "/\\:*?\"<>|".contains(character) { '_' } else { character }).collect::<String>().trim_matches('.').trim().to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn names() {
        assert_eq!(clean_name("  회사   업무 "), Some("회사 업무".to_owned()));
        assert_eq!(clean_name("   "), None);
        assert_eq!(clean_name(&"가".repeat(NAME_LIMIT)), Some("가".repeat(NAME_LIMIT)));
        assert_eq!(clean_name(&"가".repeat(NAME_LIMIT + 1)), None);
        assert_eq!(folder_safe("../a/b:c"), "_a_b_c");
    }

    #[test]
    fn normalize() {
        assert_eq!(normalized(&[]), [Profile::default_profile()], "비어 있으면 기본 하나");
        let work = Profile::new("회사").unwrap();
        let list = normalized(&[
            work.clone(),
            Profile { id: "default".into(), name: "내 것".into(), icon: GroupIcon::Symbol("star.fill".into()), color: GroupColor::Red },
            work.clone(),
            Profile { id: "../../etc".into(), ..Profile::new("나쁜 값").unwrap() },
            Profile { id: Id::new().to_string().to_lowercase(), ..Profile::new("소문자 id").unwrap() },
        ]);
        assert_eq!(list[0].icon, GroupIcon::Symbol("star.fill".into()), "기본 프로필도 아이콘 · 색을 바꿀 수 있다");
        assert_eq!(list[0].color, GroupColor::Red);
        let old: Profile = serde_json::from_str(r#"{"id":"default","name":"기본"}"#).unwrap();
        assert_eq!((old.icon, old.color), (Profile::default_icon(), Profile::default_color()), "아이콘이 없던 설정도 읽는다");
        assert_eq!(list.iter().map(|profile| profile.name.as_str()).collect::<Vec<_>>(), ["내 것", "회사"], "기본이 맨 앞, 겹치거나 폴더로 못 쓰는 id 는 뺀다");
        assert_eq!(find(&list, &work.id), work);
        assert!(find(&list, "없는 id").is_default());
    }

    #[test]
    fn folders_and_retire() {
        let root = tempfile::tempdir().unwrap();
        let base = root.path();
        let default = Profile::default_profile();
        assert_eq!(default.data_file(base), base.join("data.json"), "기본 프로필은 지금까지의 data.json");
        assert_eq!(default.backup_directory(base), base.join("backups"));
        let work = Profile::new("회사/업무").unwrap();
        assert_eq!(work.data_file(base), base.join("profiles").join(&work.id).join("data.json"));
        assert_eq!(work.backup_directory(base), base.join("profiles").join(&work.id).join("backups"));

        assert!(default.retire(base, "x").is_err(), "기본 프로필은 지울 수 없다");
        assert_eq!(work.retire(base, "20261112-120000").unwrap(), None, "저장한 적 없으면 옮길 것이 없다");
        fs::create_dir_all(work.backup_directory(base)).unwrap();
        fs::write(work.data_file(base), "{}").unwrap();
        let moved = work.retire(base, "20261112-120000").unwrap().unwrap();
        assert_eq!(moved, base.join("backups").join("deleted-profiles").join("20261112-120000 회사_업무"));
        assert!(moved.join("data.json").is_file() && moved.join("backups").is_dir());
        assert!(!work.directory(base).exists());
        // 날짜 백업만 지우는 정리가 옮긴 폴더를 건드리지 않는다
        assert!(Backups::expired(&base.join("backups"), 1, chrono::NaiveDate::from_ymd_opt(2030, 1, 1).unwrap()).is_empty());
    }
}
