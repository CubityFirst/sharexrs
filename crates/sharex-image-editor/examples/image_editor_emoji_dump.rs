//! Saves the sticker bitmap an emoji annotation shows
//! (`RenderStickerBitmap`), or with `--raw` the text drawn before it's
//! trimmed and fitted (`RenderWithDirect2D`), as the editor harness's
//! `--emoji` and `--emoji-raw` do for ShareX: premultiplied BGRA after an
//! 8-byte width/height header.
//!
//!     cargo run -p sharex-image-editor --example image_editor_emoji_dump -- <sequence> <size> <out.bgra>
//!     cargo run -p sharex-image-editor --example image_editor_emoji_dump -- --raw <sequence> <canvas> <font size> <out.bgra>

use sharex_image_editor::render::emoji;

fn main() {
    let args: Vec<String> = std::env::args().collect();
    let (pm, out) = if args[1] == "--raw" {
        let text = emoji::to_glyph(&args[2]);
        (emoji::render_raw(&text, args[3].parse().unwrap(), args[4].parse().unwrap()), &args[5])
    } else {
        (emoji::sticker_bitmap(&args[1], args[2].parse().unwrap()).map(|p| (*p).clone()), &args[3])
    };
    let Some(pm) = pm else {
        eprintln!("none");
        std::process::exit(1);
    };
    let mut bytes = Vec::new();
    bytes.extend_from_slice(&(pm.width() as i32).to_le_bytes());
    bytes.extend_from_slice(&(pm.height() as i32).to_le_bytes());
    for p in pm.to_premul_rgba().chunks_exact(4) {
        bytes.extend_from_slice(&[p[2], p[1], p[0], p[3]]);
    }
    std::fs::write(out, bytes).unwrap();
    println!("{}x{}", pm.width(), pm.height());
}
