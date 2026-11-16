use crate::ids::Id;
use crate::models::LinkGroup;
use image::imageops::FilterType;
use image::ImageFormat;
use serde::{Deserialize, Deserializer, Serialize};
use std::collections::HashSet;
use std::fs;
use std::path::{Path, PathBuf};

/// 그룹 카드 왼쪽에 보이는 아이콘. 기본 모양 중에서 고르거나 사용자가 넣은 이미지를 쓴다.
///
/// data.json 에는 `{"symbol": "folder.fill"}` 또는 `{"image": "<파일 이름>.png"}` 로 저장한다.
/// 기본 모양의 이름은 Swift 판과 같은 SF Symbol 이름을 그대로 쓰고(data.json 호환), 그림은 앱에 든 SVG 로 그린다.
/// 이미지는 앱 데이터 폴더의 icons/ 에 복사해 두므로 원본 파일을 지우거나 옮겨도 아이콘이 남는다.
#[derive(Clone, Debug, PartialEq, Eq, Hash, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum GroupIcon {
    Symbol(String),
    Image(String),
}

impl GroupIcon {
    pub const DEFAULT_SYMBOL: &'static str = "folder.fill";

    /// 그룹 편집 창에서 고를 수 있는 기본 모양 (한 줄 8개)
    pub const PRESET_SYMBOLS: [&'static str; 40] = [
        // 개발
        "folder.fill",
        "chevron.left.forwardslash.chevron.right",
        "curlybraces",
        "terminal.fill",
        "arrow.triangle.branch",
        "ladybug.fill",
        "hammer.fill",
        "wrench.and.screwdriver.fill",
        // 인프라 · 데이터
        "display",
        "server.rack",
        "cylinder.split.1x2.fill",
        "externaldrive.fill",
        "cpu",
        "network",
        "cloud.fill",
        "shippingbox.fill",
        // 지표 · 문서 · 일정
        "chart.xyaxis.line",
        "chart.bar.fill",
        "chart.pie.fill",
        "doc.text.fill",
        "checklist",
        "list.bullet.clipboard.fill",
        "calendar",
        "briefcase.fill",
        // 소통 · 설정 · 운영
        "envelope.fill",
        "bubble.left.fill",
        "person.2.fill",
        "gearshape.fill",
        "lock.fill",
        "key.fill",
        "exclamationmark.triangle.fill",
        "flag.fill",
        // 그 밖
        "newspaper.fill",
        "book.fill",
        "graduationcap.fill",
        "globe",
        "house.fill",
        "gamecontroller.fill",
        "star.fill",
        "heart.fill",
    ];

    pub fn image_file_name(&self) -> Option<&str> {
        match self {
            GroupIcon::Image(name) => Some(name),
            GroupIcon::Symbol(_) => None,
        }
    }
}

impl Default for GroupIcon {
    fn default() -> Self {
        GroupIcon::Symbol(Self::DEFAULT_SYMBOL.to_owned())
    }
}

/// image 가 있으면 이미지, 없으면 symbol 을 읽는다. 둘 다 없으면 읽지 못한다.
impl<'de> Deserialize<'de> for GroupIcon {
    fn deserialize<D: Deserializer<'de>>(deserializer: D) -> Result<Self, D::Error> {
        #[derive(Deserialize)]
        struct Raw {
            #[serde(default)]
            image: Option<String>,
            #[serde(default)]
            symbol: Option<String>,
        }

        let raw = Raw::deserialize(deserializer)?;
        if let Some(name) = raw.image {
            return Ok(GroupIcon::Image(name));
        }
        raw.symbol.map(GroupIcon::Symbol).ok_or_else(|| serde::de::Error::missing_field("symbol"))
    }
}

#[derive(Debug, thiserror::Error)]
pub enum GroupIconStoreError {
    #[error("{}", crate::tr!("이미지를 읽지 못했습니다. PNG · JPG · WebP 같은 이미지 파일을 골라 주세요.", "Couldn't read the image. Choose an image file such as PNG, JPG or WebP."))]
    UnreadableImage,
    #[error("{}", crate::tr!("이미지를 저장하지 못했습니다: {}", "Couldn't save the image: {}", .0))]
    Io(#[from] std::io::Error),
}

/// 사용자가 고른 그룹 아이콘 이미지를 앱 데이터 폴더 icons/ 에 정사각형 PNG 로 보관한다.
pub struct GroupIconStore;

impl GroupIconStore {
    /// 저장하는 이미지 한 변 픽셀. 카드 아이콘(최대 44pt)의 2배 해상도를 넉넉히 덮는다.
    pub const PIXEL_SIZE: u32 = 128;

    pub fn directory(data_directory: &Path) -> PathBuf {
        data_directory.join("icons")
    }

    /// data.json 에 적힌 이름으로 파일 위치를 찾는다. 다른 폴더를 가리키는 이름(경로 · 숨김 파일)은 받지 않는다.
    pub fn file_path(name: &str, data_directory: &Path) -> Option<PathBuf> {
        if name.is_empty() || name.starts_with('.') || name.contains(['/', '\\']) {
            return None;
        }
        if Path::new(name).file_name().and_then(|file| file.to_str()) != Some(name) {
            return None;
        }
        Some(Self::directory(data_directory).join(name))
    }

    /// 이미지를 가운데 기준 정사각형으로 잘라 줄인 뒤 새 이름으로 저장하고 그 이름을 돌려준다.
    pub fn import_image(source: &Path, data_directory: &Path) -> Result<String, GroupIconStoreError> {
        let bytes = fs::read(source).map_err(|_| GroupIconStoreError::UnreadableImage)?;
        let png = Self::square_png(&bytes).ok_or(GroupIconStoreError::UnreadableImage)?;
        let folder = Self::directory(data_directory);
        fs::create_dir_all(&folder)?;
        let name = format!("{}.png", Id::new());
        crate::fsutil::write_atomic(&folder.join(&name), &png)?;
        Ok(name)
    }

    /// 어느 그룹도 쓰지 않는 이미지(아이콘을 바꿨거나 그룹을 지웠거나 편집을 취소한 경우)를 지운다.
    pub fn remove_unused(keeping: &[LinkGroup], data_directory: &Path) -> Vec<String> {
        let used: HashSet<&str> = keeping.iter().filter_map(|group| group.icon.image_file_name()).collect();
        Self::remove_unused_names(&used, data_directory)
    }

    /// used 에 없는 아이콘 이미지를 지운다 (그룹 말고 프로필 아이콘도 이 폴더를 쓴다).
    pub fn remove_unused_names(used: &HashSet<&str>, data_directory: &Path) -> Vec<String> {
        let folder = Self::directory(data_directory);
        let Ok(entries) = fs::read_dir(&folder) else {
            return Vec::new();
        };
        let mut unused: Vec<String> = entries
            .filter_map(|entry| entry.ok()?.file_name().into_string().ok())
            .filter(|name| name.ends_with(".png") && !used.contains(name.as_str()))
            .collect();
        unused.sort();
        for name in &unused {
            let _ = fs::remove_file(folder.join(name));
        }
        unused
    }

    fn square_png(bytes: &[u8]) -> Option<Vec<u8>> {
        let image = image::load_from_memory(bytes).ok()?;
        let side = image.width().min(image.height());
        if side == 0 {
            return None;
        }
        let cropped = image.crop_imm((image.width() - side) / 2, (image.height() - side) / 2, side, side);
        let resized = cropped.resize_exact(Self::PIXEL_SIZE, Self::PIXEL_SIZE, FilterType::Lanczos3).into_rgba8();
        let mut png = std::io::Cursor::new(Vec::new());
        resized.write_to(&mut png, ImageFormat::Png).ok()?;
        Some(png.into_inner())
    }
}
