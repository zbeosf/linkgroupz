//! 운영체제마다 다른 부분: 브라우저 찾기 · 실행, 실행 중인지, 앱 아이콘, 비밀 값 보관.

use image::imageops::FilterType;
use image::{ImageFormat, RgbaImage};

#[cfg(target_os = "macos")]
mod macos;
#[cfg(target_os = "macos")]
pub use macos::*;

#[cfg(windows)]
mod windows;
#[cfg(windows)]
pub use windows::*;

#[cfg(not(any(target_os = "macos", windows)))]
mod other;
#[cfg(not(any(target_os = "macos", windows)))]
pub use other::*;

/// 아이콘을 size×size PNG 로 맞춘다.
#[allow(dead_code)]
pub(crate) fn icon_to_png(image: RgbaImage, size: u32) -> Option<Vec<u8>> {
    if image.width() == 0 || image.height() == 0 || size == 0 {
        return None;
    }
    let resized = if image.width() == size && image.height() == size {
        image
    } else {
        image::imageops::resize(&image, size, size, FilterType::Lanczos3)
    };
    let mut png = std::io::Cursor::new(Vec::new());
    resized.write_to(&mut png, ImageFormat::Png).ok()?;
    Some(png.into_inner())
}
