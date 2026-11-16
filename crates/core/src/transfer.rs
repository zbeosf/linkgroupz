//! 데이터 내보내기 · 가져오기.
//!
//! 내보내기는 지금 data.json 과 같은 내용을 고른 파일에 쓴다. 가져오기는 data.json 형식(원본 LinkGroups 의 것도 같다)을
//! 읽어 지금 데이터를 바꾸거나(replace), 지금 목록 끝에 더한다(merge). 바꾸기 전 데이터는 백업 폴더에 따로 남긴다.
//!
//! 그룹 아이콘으로 넣은 내 이미지(icons/ 의 PNG, 하나에 보통 수십 KB)는 같은 파일의 `iconImages` 칸에
//! { 파일 이름: base64 } 로 함께 넣는다. 이 칸을 모르는 앱(원본 LinkGroups 등)은 무시하고 그룹만 읽는다.

use crate::archive::LinkArchive;
use crate::base64url::Base64Url;
use crate::fsutil::write_atomic;
use crate::group_icon::GroupIconStore;
use crate::ids::Id;
use crate::models::{LinkGroupsDocument, SidebarItem};
use crate::store::{sorted_keys, LinkStore};
use serde::Serialize;
use serde_json::{Map, Value};
use std::collections::{HashMap, HashSet};
use std::fs;
use std::path::Path;

/// 내보낸 파일에서 아이콘 이미지를 담는 칸
const ICON_IMAGES_KEY: &str = "iconImages";

#[derive(Debug, thiserror::Error)]
pub enum TransferError {
    #[error("{}", crate::tr!("LinkGroupz(또는 LinkGroups) 데이터 파일이 아닙니다: {}", "Not a LinkGroupz (or LinkGroups) data file: {}", .0))]
    NotADataFile(String),
    #[error("{}", crate::tr!("이 앱보다 새 버전이 만든 데이터 파일이라 읽을 수 없습니다 (형식 {}).", "This data file was made by a newer version of the app and can't be read (format {}).", .0))]
    UnsupportedVersion(i64),
}

/// 가져올 파일에 무엇이 들었는지 (가져오기 전에 보여 준다)
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSummary {
    pub groups: usize,
    pub links: usize,
    pub sections: usize,
    pub archived: usize,
    /// 함께 든 아이콘 이미지 수
    pub icon_images: usize,
}

impl ImportSummary {
    pub fn of(document: &LinkGroupsDocument) -> Self {
        Self {
            groups: document.groups.len(),
            links: document.groups.iter().map(|group| group.links.len()).sum(),
            sections: document.sections.len(),
            archived: document.archive.count(),
            icon_images: 0,
        }
    }

    /// 파일 내용까지 보고 (아이콘 이미지 수 포함)
    pub fn of_file(document: &LinkGroupsDocument, data: &[u8]) -> Self {
        Self { icon_images: icon_images_in(data).len(), ..Self::of(document) }
    }
}

/// data.json 형식의 내용에, 그룹(보관함 포함)이 쓰는 아이콘 이미지를 `iconImages` 칸으로 붙인다.
/// 이미지 파일이 없으면 그 이미지는 빼고, 내용을 읽지 못하면 받은 그대로 돌려준다.
pub fn with_icon_images(data: &[u8], data_directory: &Path) -> Vec<u8> {
    let (Ok(document), Ok(Value::Object(mut object))) = (serde_json::from_slice::<LinkGroupsDocument>(data), serde_json::from_slice::<Value>(data)) else {
        return data.to_vec();
    };
    let names: HashSet<String> = document
        .groups
        .iter()
        .chain(document.archive.groups.iter().map(|item| &item.group))
        .filter_map(|group| group.icon.image_file_name().map(str::to_owned))
        .collect();
    let mut images = Map::new();
    let mut names: Vec<String> = names.into_iter().collect();
    names.sort();
    for name in names {
        if let Some(bytes) = GroupIconStore::file_path(&name, data_directory).and_then(|path| fs::read(path).ok()) {
            images.insert(name, Value::String(Base64Url::encode(&bytes)));
        }
    }
    object.remove(ICON_IMAGES_KEY);
    if !images.is_empty() {
        object.insert(ICON_IMAGES_KEY.to_owned(), Value::Object(images));
    }
    serde_json::to_vec_pretty(&sorted_keys(Value::Object(object))).unwrap_or_else(|_| data.to_vec())
}

/// 파일에 든 아이콘 이미지 (이름, PNG). 이름이 폴더 밖을 가리키거나 PNG 가 아니면 뺀다.
fn icon_images_in(data: &[u8]) -> Vec<(String, Vec<u8>)> {
    let Ok(Value::Object(object)) = serde_json::from_slice::<Value>(data) else {
        return Vec::new();
    };
    let Some(Value::Object(images)) = object.get(ICON_IMAGES_KEY) else {
        return Vec::new();
    };
    images
        .iter()
        .filter(|(name, _)| GroupIconStore::file_path(name, Path::new("")).is_some() && name.ends_with(".png"))
        .filter_map(|(name, value)| Some((name.clone(), Base64Url::decode(value.as_str()?)?)))
        .filter(|(_, bytes)| image::load_from_memory_with_format(bytes, image::ImageFormat::Png).is_ok())
        .collect()
}

/// 파일에 든 아이콘 이미지를 앱 데이터 폴더 icons/ 에 되살린다. 같은 이름이 이미 있으면 그대로 둔다. 되살린 수를 돌려준다.
pub fn restore_icon_images(data: &[u8], data_directory: &Path) -> usize {
    let images = icon_images_in(data);
    if images.is_empty() || fs::create_dir_all(GroupIconStore::directory(data_directory)).is_err() {
        return 0;
    }
    images
        .into_iter()
        .filter_map(|(name, bytes)| {
            let path = GroupIconStore::file_path(&name, data_directory)?;
            (!path.exists() && write_atomic(&path, &bytes).is_ok()).then_some(())
        })
        .count()
}

/// 가져오는 방법
#[derive(Clone, Copy, Debug, PartialEq, Eq, serde::Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportMode {
    /// 지금 데이터를 파일 내용으로 바꾼다
    Replace,
    /// 지금 목록 끝에 파일의 그룹 · 섹션 · 보관함을 더한다
    Merge,
}

/// 파일 내용을 데이터로 읽는다.
pub fn read_document(data: &[u8]) -> Result<LinkGroupsDocument, TransferError> {
    let document: LinkGroupsDocument =
        serde_json::from_slice(data).map_err(|error| TransferError::NotADataFile(error.to_string()))?;
    if document.version > LinkGroupsDocument::CURRENT_VERSION {
        return Err(TransferError::UnsupportedVersion(document.version));
    }
    Ok(document)
}

impl LinkStore {
    /// 지금 데이터를 data.json 과 같은 모양으로 (내보내기)
    pub fn export_data(&self) -> Vec<u8> {
        let mut document = LinkGroupsDocument::new(self.groups.clone(), self.archive.clone());
        document.sections = self.sections.clone();
        document.sidebar_order = self.sidebar_order.clone();
        let value = serde_json::to_value(&document).map(sorted_keys).unwrap_or_default();
        serde_json::to_vec_pretty(&value).unwrap_or_default()
    }

    /// 가져온 데이터로 바꾸거나 더한다.
    pub fn import_document(&mut self, document: LinkGroupsDocument, mode: ImportMode) {
        match mode {
            ImportMode::Replace => {
                self.groups = document.groups;
                self.archive = document.archive;
                self.sections = document.sections;
                self.sidebar_order = document.sidebar_order;
            }
            ImportMode::Merge => self.merge(document),
        }
        self.mutate_all(|_, _| {});
    }

    /// 끝에 더한다. 지금 데이터와 id 가 겹치는 그룹 · 섹션 · 페이지는 새 id 를 붙여 둘 다 남긴다.
    fn merge(&mut self, document: LinkGroupsDocument) {
        // 섹션이 없던 목록이면 지금 순서부터 맞춘다 (그룹 목록 순서 그대로).
        self.normalize_sections();
        if self.sections.is_empty() {
            self.sidebar_order = self.groups.iter().map(|group| SidebarItem::Group(group.id)).collect();
        }
        let mut used: HashSet<Id> = self.groups.iter().map(|group| group.id).collect();
        used.extend(self.sections.iter().map(|section| section.id));
        used.extend(self.groups.iter().flat_map(|group| group.links.iter().map(|link| link.id)));
        used.extend(self.archive.groups.iter().map(|item| item.id()));
        used.extend(self.archive.links.iter().map(|item| item.id()));
        let fresh = |id: Id, used: &mut HashSet<Id>| -> Id {
            if used.insert(id) {
                return id;
            }
            let new = Id::new();
            used.insert(new);
            new
        };

        let mut section_ids: HashMap<Id, Id> = HashMap::new();
        for mut section in document.sections {
            let new = fresh(section.id, &mut used);
            section_ids.insert(section.id, new);
            section.id = new;
            self.sections.push(section);
        }
        let mut group_ids: HashMap<Id, Id> = HashMap::new();
        for mut group in document.groups {
            let new = fresh(group.id, &mut used);
            group_ids.insert(group.id, new);
            group.id = new;
            group.section_id = group.section_id.and_then(|id| section_ids.get(&id).copied());
            for link in &mut group.links {
                link.id = fresh(link.id, &mut used);
            }
            self.groups.push(group);
        }
        // 가져온 파일의 맨 위 순서대로 끝에 붙인다 (없거나 빠진 것은 정리할 때 끝에 붙는다).
        for item in document.sidebar_order {
            let mapped = match item {
                SidebarItem::Section(id) => section_ids.get(&id).copied().map(SidebarItem::Section),
                SidebarItem::Group(id) => group_ids.get(&id).copied().map(SidebarItem::Group),
            };
            if let Some(mapped) = mapped {
                self.sidebar_order.push(mapped);
            }
        }
        let LinkArchive { groups, links } = document.archive;
        for mut item in groups {
            item.group.id = fresh(item.group.id, &mut used);
            item.group.section_id = None;
            self.archive.groups.push(item);
        }
        for mut item in links {
            item.link.id = fresh(item.link.id, &mut used);
            self.archive.links.push(item);
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::group_icon::GroupIcon;
    use crate::models::LinkGroup;

    /// 1×1 PNG
    fn png() -> Vec<u8> {
        let mut bytes = std::io::Cursor::new(Vec::new());
        image::RgbaImage::new(1, 1).write_to(&mut bytes, image::ImageFormat::Png).unwrap();
        bytes.into_inner()
    }

    #[test]
    fn icon_images_travel_with_the_file() {
        let from = tempfile::tempdir().unwrap();
        let to = tempfile::tempdir().unwrap();
        fs::create_dir_all(GroupIconStore::directory(from.path())).unwrap();
        fs::write(GroupIconStore::file_path("mine.png", from.path()).unwrap(), png()).unwrap();

        let mut store = LinkStore::open(from.path().join("data.json"));
        let mut group = LinkGroup::new("내 아이콘");
        group.icon = GroupIcon::Image("mine.png".into());
        store.add_group(group);
        let mut missing = LinkGroup::new("파일 없는 아이콘");
        missing.icon = GroupIcon::Image("gone.png".into());
        store.add_group(missing);

        let exported = with_icon_images(&store.export_data(), from.path());
        let document = read_document(&exported).unwrap();
        assert_eq!(document.groups.len(), 2, "이미지 칸이 있어도 그룹은 그대로 읽는다");
        assert_eq!(ImportSummary::of_file(&document, &exported).icon_images, 1, "없는 이미지 파일은 빼고 넣는다");

        assert_eq!(restore_icon_images(&exported, to.path()), 1);
        assert_eq!(fs::read(GroupIconStore::file_path("mine.png", to.path()).unwrap()).unwrap(), png());
        assert_eq!(restore_icon_images(&exported, to.path()), 0, "이미 있는 이미지는 덮어쓰지 않는다");
    }

    #[test]
    fn odd_icon_images_are_ignored() {
        let to = tempfile::tempdir().unwrap();
        let png = Base64Url::encode(&png());
        let file = serde_json::json!({
            "version": 1,
            "groups": [],
            "iconImages": {
                "../escape.png": png,
                ".hidden.png": png,
                "not-png.png": Base64Url::encode(b"hello"),
                "picture.jpg": png,
                "ok.png": png,
            }
        });
        let data = serde_json::to_vec(&file).unwrap();
        assert_eq!(restore_icon_images(&data, to.path()), 1, "폴더 밖 · 숨김 · PNG 가 아닌 것은 넣지 않는다");
        assert!(GroupIconStore::directory(to.path()).join("ok.png").is_file());
        assert!(!to.path().join("escape.png").exists());
    }
}
