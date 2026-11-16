use crate::{workspace_root, Outcome};
use image::codecs::ico::{IcoEncoder, IcoFrame};
use image::{ExtendedColorType, RgbaImage};
use resvg::tiny_skia::{Pixmap, Transform};
use resvg::usvg::{Options, Tree};
use std::fs::{self, File};
use std::io::BufWriter;
use std::path::Path;

/// SVG 를 width×height 픽셀 그림으로 그린다.
fn render(svg: &Path, width: u32, height: u32) -> Outcome<RgbaImage> {
    let tree = Tree::from_data(&fs::read(svg)?, &Options::default())?;
    let mut pixmap = Pixmap::new(width, height).ok_or("그림 크기가 0 입니다")?;
    let size = tree.size();
    let transform = Transform::from_scale(width as f32 / size.width(), height as f32 / size.height());
    resvg::render(&tree, transform, &mut pixmap.as_mut());
    // tiny-skia 는 알파를 미리 곱한 색으로 들고 있다. PNG 에 쓰려면 되돌려야 한다.
    let pixels: Vec<u8> = pixmap
        .pixels()
        .iter()
        .flat_map(|pixel| {
            let color = pixel.demultiply();
            [color.red(), color.green(), color.blue(), color.alpha()]
        })
        .collect();
    Ok(RgbaImage::from_raw(width, height, pixels).ok_or("그림 버퍼 크기가 맞지 않습니다")?)
}

/// assets/app-icon.svg · assets/tray-icon.svg 로 앱이 쓰는 그림을 모두 만든다.
pub fn generate() -> Outcome {
    let root = workspace_root();
    let app_svg = root.join("assets/app-icon.svg");
    let tray_svg = root.join("assets/tray-icon.svg");
    let out = root.join("crates/app/icons");
    fs::create_dir_all(&out)?;

    // 앱 아이콘 (Tauri 번들 설정이 가리키는 파일들)
    for (name, size) in [("32x32.png", 32), ("128x128.png", 128), ("128x128@2x.png", 256), ("icon.png", 512)] {
        render(&app_svg, size, size)?.save(out.join(name))?;
    }

    // macOS: icon.icns
    let mut family = icns::IconFamily::new();
    for size in [16, 32, 64, 128, 256, 512, 1024] {
        let image = render(&app_svg, size, size)?;
        family.add_icon(&icns::Image::from_data(icns::PixelFormat::RGBA, size, size, image.into_raw())?)?;
    }
    family.write(BufWriter::new(File::create(out.join("icon.icns"))?))?;

    // Windows: icon.ico
    let mut frames = Vec::new();
    for size in [16, 24, 32, 48, 64, 256] {
        let image = render(&app_svg, size, size)?;
        frames.push(IcoFrame::as_png(image.as_raw(), size, size, ExtendedColorType::Rgba8)?);
    }
    IcoEncoder::new(BufWriter::new(File::create(out.join("icon.ico"))?)).encode_images(&frames)?;

    // 메뉴바 아이콘: macOS 는 템플릿(검정 + 투명) 그림을 18pt 높이로 줄여 쓴다. 선명하도록 4배로 그려 둔다.
    render(&tray_svg, 72, 64)?.save(out.join("tray-template.png"))?;
    // Windows 트레이는 색이 있는 작은 앱 아이콘을 쓴다.
    render(&app_svg, 64, 64)?.save(out.join("tray-color.png"))?;

    // 화면(시작 안내 · 바뀐 점)에 넣는 앱 아이콘
    render(&app_svg, 256, 256)?.save(root.join("crates/app/ui/app-icon.png"))?;

    // 사용법 문서에 넣는 앱 아이콘
    let guide = root.join("docs/guide/images");
    if guide.is_dir() {
        render(&app_svg, 256, 256)?.save(guide.join("app-icon.png"))?;
    }

    println!("아이콘: {}", out.display());
    Ok(())
}
