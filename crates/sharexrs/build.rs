//! Puts ShareXrs's icon in sharexrs.exe, from `assets/ShareXrs.iconset`
//! (scripts/make-icons.sh): it's what Explorer, the Start menu and taskbar
//! pins show, which can't use the icon the app sets on its windows.

use std::path::Path;

fn main() {
    if std::env::var("CARGO_CFG_TARGET_OS").as_deref() != Ok("windows") {
        return;
    }
    let dir = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../assets/ShareXrs.iconset");
    println!("cargo:rerun-if-changed={}", dir.display());
    if let Err(e) = windows_icon(&dir) {
        println!("cargo:warning=sharexrs.exe built without its icon: {e}");
    }
}

fn windows_icon(dir: &Path) -> Result<(), String> {
    let files = ["icon_16x16.png", "icon_32x32.png", "icon_32x32@2x.png", "icon_128x128.png", "icon_256x256.png"];
    let pngs = files
        .iter()
        .map(|f| std::fs::read(dir.join(f)).map_err(|e| format!("{f}: {e}")))
        .collect::<Result<Vec<_>, _>>()?;
    let out = Path::new(&std::env::var("OUT_DIR").unwrap()).to_path_buf();
    let ico = out.join("sharexrs.ico");
    std::fs::write(&ico, ico_of(&pngs)).map_err(|e| e.to_string())?;
    let rc = out.join("sharexrs.rc");
    let path = ico.to_string_lossy().replace('\\', "\\\\");
    std::fs::write(&rc, format!("1 ICON \"{path}\"\n")).map_err(|e| e.to_string())?;
    embed_resource::compile(&rc, embed_resource::NONE).manifest_optional().map_err(|e| e.to_string())
}

/// An .ico holding PNGs as they are (which Windows has read since Vista).
fn ico_of(pngs: &[Vec<u8>]) -> Vec<u8> {
    let mut ico = vec![0, 0, 1, 0];
    ico.extend((pngs.len() as u16).to_le_bytes());
    let mut offset = 6 + 16 * pngs.len() as u32;
    for png in pngs {
        // The PNG's IHDR has its width; 0 means 256 here.
        let side = u32::from_be_bytes(png[16..20].try_into().unwrap());
        let side = if side >= 256 { 0 } else { side as u8 };
        ico.extend([side, side, 0, 0, 1, 0, 32, 0]);
        ico.extend((png.len() as u32).to_le_bytes());
        ico.extend(offset.to_le_bytes());
        offset += png.len() as u32;
    }
    for png in pngs {
        ico.extend(png);
    }
    ico
}
