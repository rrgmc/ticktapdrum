//! Writes every platform's icon files from the two icon drawings in `docs/icon/`:
//!
//! - iOS: `AppIcon.appiconset/icon-1024.png`, opaque, square (iOS rounds the corners itself).
//! - Desktop: `apps/dm-desktop/assets/icon-256.png` (window and Linux icon), `icon.ico` (Windows
//!   program and installer) and `icon-1024.png` (the macOS `.icns`), with rounded corners.
//! - The interface: `crates/dm-ui/assets/logo-256.png`, the logo on the bank's download page,
//!   the same as the desktop icon.
//! - Android: the adaptive icon's foreground and monochrome layers in each `mipmap-<density>`
//!   folder, and its background colour in `values/ic_launcher_background.xml`.
//!
//! Run it after a drawing changes, and commit the files it writes:
//! `cargo run -p dm-icons --release`.

use std::error::Error;
use std::fs;
use std::io::BufWriter;
use std::path::Path;

use resvg::tiny_skia::{FillRule, Mask, PathBuilder, Pixmap, Transform};
use resvg::usvg::{Options, Tree};

type Result<T> = std::result::Result<T, Box<dyn Error>>;

/// The drawings' canvas: the 108 x 108 Android adaptive-icon canvas.
const CANVAS: f32 = 108.0;

/// The square of the canvas that the iOS and desktop icons show, as (left, top, side) in canvas
/// units. It is the middle 72 x 72 that Android launchers show, so the drum fills every platform's
/// icon alike.
const CROP: (f32, f32, f32) = (18.0, 18.0, 72.0);

/// The desktop icons' corner radius, as a share of the side.
const CORNER: f32 = 0.22;

/// The sizes in the Windows `.ico` file.
const ICO_SIZES: [u32; 7] = [16, 24, 32, 48, 64, 128, 256];

/// Android's densities and the side of the 108 dp canvas at each.
const DENSITIES: [(&str, u32); 5] = [
    ("mdpi", 108),
    ("hdpi", 162),
    ("xhdpi", 216),
    ("xxhdpi", 324),
    ("xxxhdpi", 432),
];

/// The full-canvas background rectangle of the colour drawing, up to its colour.
const BACKGROUND_START: &str = r#"<rect width="108" height="108" fill=""#;

fn main() -> Result<()> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let colour_svg = fs::read_to_string(root.join("docs/icon/ticktapdrum.svg"))?;
    let mono_svg = fs::read_to_string(root.join("docs/icon/ticktapdrum-mono.svg"))?;
    let colour = parse(&colour_svg)?;

    // iOS.
    let ios = root.join("ports/ios/TickTapDrum/Assets.xcassets/AppIcon.appiconset/icon-1024.png");
    write_opaque_png(&render_crop(&colour, 1024)?, &ios)?;

    // Desktop.
    let assets = root.join("apps/dm-desktop/assets");
    write_png(
        &rounded(render_crop(&colour, 256)?)?,
        &assets.join("icon-256.png"),
    )?;
    write_png(
        &rounded(render_crop(&colour, 1024)?)?,
        &assets.join("icon-1024.png"),
    )?;
    let mut ico_images = Vec::new();
    for size in ICO_SIZES {
        ico_images.push((size, rounded(render_crop(&colour, size)?)?.encode_png()?));
    }
    write(&assets.join("icon.ico"), &ico(&ico_images))?;

    // The logo the interface shows, as the desktop icon.
    write_png(
        &rounded(render_crop(&colour, 256)?)?,
        &root.join("crates/dm-ui/assets/logo-256.png"),
    )?;

    // Android: the background colour goes in its own layer, so the foreground leaves it out.
    let (background, foreground_svg) = split_background(&colour_svg)?;
    let foreground = parse(&foreground_svg)?;
    let mono = parse(&mono_svg)?;
    let res = root.join("ports/android/app/src/main/res");
    for (density, side) in DENSITIES {
        let dir = res.join(format!("mipmap-{density}"));
        write_png(
            &render(&foreground, (0.0, 0.0, CANVAS), side)?,
            &dir.join("ic_launcher_foreground.png"),
        )?;
        write_png(
            &render(&mono, (0.0, 0.0, CANVAS), side)?,
            &dir.join("ic_launcher_monochrome.png"),
        )?;
    }
    let colour_xml = format!(
        "<?xml version=\"1.0\" encoding=\"utf-8\"?>\n\
         <!-- The dm-icons tool (tools/icons) writes this file from docs/icon/ticktapdrum.svg. -->\n\
         <resources>\n    <color name=\"ic_launcher_background\">{background}</color>\n</resources>\n"
    );
    write(
        &res.join("values/ic_launcher_background.xml"),
        colour_xml.as_bytes(),
    )?;
    Ok(())
}

fn parse(svg: &str) -> Result<Tree> {
    Ok(Tree::from_str(svg, &Options::default())?)
}

/// Returns the colour of the drawing's full-canvas background rectangle, and the drawing without
/// that rectangle.
fn split_background(svg: &str) -> Result<(String, String)> {
    let start = svg
        .find(BACKGROUND_START)
        .ok_or("the colour drawing has no full-canvas background rectangle")?;
    let rest = &svg[start + BACKGROUND_START.len()..];
    let colour_end = rest.find('"').ok_or("unterminated background colour")?;
    let element_end = rest.find("/>").ok_or("unterminated background rectangle")? + 2;
    let colour = rest[..colour_end].to_string();
    let without = format!("{}{}", &svg[..start], &rest[element_end..]);
    Ok((colour, without))
}

/// Renders the iOS and desktop square of the drawing at `side` x `side` pixels.
fn render_crop(tree: &Tree, side: u32) -> Result<Pixmap> {
    render(tree, CROP, side)
}

/// Renders the square `(left, top, size)` of the canvas at `side` x `side` pixels.
fn render(tree: &Tree, (left, top, size): (f32, f32, f32), side: u32) -> Result<Pixmap> {
    let mut pixmap = Pixmap::new(side, side).ok_or("empty image")?;
    // The drawings' viewBox is the canvas; usvg maps it to the drawing's width and height.
    let scale = side as f32 / size;
    let to_canvas = CANVAS / tree.size().width();
    let transform = Transform::from_scale(scale, scale)
        .pre_translate(-left, -top)
        .pre_scale(to_canvas, to_canvas);
    resvg::render(tree, transform, &mut pixmap.as_mut());
    Ok(pixmap)
}

/// Clears the corners outside a rounded square.
fn rounded(mut pixmap: Pixmap) -> Result<Pixmap> {
    let side = pixmap.width() as f32;
    let r = side * CORNER;
    // A quarter circle as a cubic Bezier curve: the control points sit k * r from the ends.
    let k = r * 0.552_284_8;
    let mut path = PathBuilder::new();
    path.move_to(r, 0.0);
    path.line_to(side - r, 0.0);
    path.cubic_to(side - r + k, 0.0, side, r - k, side, r);
    path.line_to(side, side - r);
    path.cubic_to(side, side - r + k, side - r + k, side, side - r, side);
    path.line_to(r, side);
    path.cubic_to(r - k, side, 0.0, side - r + k, 0.0, side - r);
    path.line_to(0.0, r);
    path.cubic_to(0.0, r - k, r - k, 0.0, r, 0.0);
    path.close();
    let path = path.finish().ok_or("empty corner path")?;
    let mut mask = Mask::new(pixmap.width(), pixmap.height()).ok_or("empty mask")?;
    mask.fill_path(&path, FillRule::Winding, true, Transform::identity());
    pixmap.apply_mask(&mask);
    Ok(pixmap)
}

/// Builds an `.ico` file from PNG images, which Windows Vista and later read.
fn ico(images: &[(u32, Vec<u8>)]) -> Vec<u8> {
    let count = images.len() as u16;
    let mut out = Vec::new();
    out.extend_from_slice(&0u16.to_le_bytes()); // reserved
    out.extend_from_slice(&1u16.to_le_bytes()); // type: icon
    out.extend_from_slice(&count.to_le_bytes());
    let mut offset = 6 + 16 * images.len() as u32;
    for (side, png) in images {
        let side = (side % 256) as u8; // 0 means 256
        out.extend_from_slice(&[side, side, 0, 0]); // width, height, no palette, reserved
        out.extend_from_slice(&1u16.to_le_bytes()); // colour planes
        out.extend_from_slice(&32u16.to_le_bytes()); // bits per pixel
        out.extend_from_slice(&(png.len() as u32).to_le_bytes());
        out.extend_from_slice(&offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for (_, png) in images {
        out.extend_from_slice(png);
    }
    out
}

fn write_png(pixmap: &Pixmap, path: &Path) -> Result<()> {
    write(path, &pixmap.encode_png()?)
}

/// Writes an RGB PNG without an alpha channel, which the App Store requires of the app icon.
fn write_opaque_png(pixmap: &Pixmap, path: &Path) -> Result<()> {
    let mut rgb = Vec::with_capacity(pixmap.pixels().len() * 3);
    for pixel in pixmap.pixels() {
        if pixel.alpha() != 255 {
            return Err(format!("{}: the image is not opaque", path.display()).into());
        }
        rgb.extend_from_slice(&[pixel.red(), pixel.green(), pixel.blue()]);
    }
    create_parent(path)?;
    let mut encoder = png::Encoder::new(
        BufWriter::new(fs::File::create(path)?),
        pixmap.width(),
        pixmap.height(),
    );
    encoder.set_color(png::ColorType::Rgb);
    encoder.set_depth(png::BitDepth::Eight);
    encoder.write_header()?.write_image_data(&rgb)?;
    println!("wrote {}", shown(path));
    Ok(())
}

fn write(path: &Path, bytes: &[u8]) -> Result<()> {
    create_parent(path)?;
    fs::write(path, bytes)?;
    println!("wrote {}", shown(path));
    Ok(())
}

fn create_parent(path: &Path) -> Result<()> {
    if let Some(parent) = path.parent() {
        fs::create_dir_all(parent)?;
    }
    Ok(())
}

/// The path relative to the repository, with forward slashes, for the output.
fn shown(path: &Path) -> String {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../..");
    let relative = path.strip_prefix(&root).unwrap_or(path);
    relative.display().to_string().replace('\\', "/")
}
