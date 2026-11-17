//! `ImageEffectPackager`: `.sxie` packages, a zip of `Config.json` (the preset in the
//! export form) and the images under an assets folder, which ShareX extracts into its
//! `ImageEffects` folder on import.

use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

const CONFIG_FILE_NAME: &str = "Config.json";
/// `ZipManager.Extract`'s limit for packages.
const MAX_UNCOMPRESSED: u64 = 100_000_000;
const IMAGE_EXTENSIONS: [&str; 7] = ["jpg", "jpeg", "png", "gif", "bmp", "ico", "webp"];

fn is_image_file(name: &str) -> bool {
    name.rsplit_once('.')
        .is_some_and(|(_, e)| IMAGE_EXTENSIONS.iter().any(|x| x.eq_ignore_ascii_case(e)))
}

fn files_recursive(dir: &Path, out: &mut Vec<PathBuf>) {
    let Ok(entries) = std::fs::read_dir(dir) else {
        return;
    };
    let mut entries: Vec<_> = entries.filter_map(|e| e.ok().map(|e| e.path())).collect();
    entries.sort();
    for p in entries {
        if p.is_dir() {
            files_recursive(&p, out);
        } else {
            out.push(p);
        }
    }
}

/// `ImageEffectPackager.Package`: writes `output` and returns it.
pub fn package(
    output: &Path,
    config_json: &str,
    assets_folder: Option<&Path>,
) -> std::io::Result<PathBuf> {
    if let Some(parent) = output.parent() {
        std::fs::create_dir_all(parent)?;
    }
    let file = std::fs::File::create(output)?;
    let mut zip = zip::ZipWriter::new(file);
    let options = zip::write::SimpleFileOptions::default()
        .compression_method(zip::CompressionMethod::Deflated);
    zip.start_file(CONFIG_FILE_NAME, options)?;
    zip.write_all(config_json.as_bytes())?;
    if let Some(assets) = assets_folder.filter(|p| p.is_dir()) {
        let parent = assets.parent().unwrap_or(assets);
        let mut files = Vec::new();
        files_recursive(assets, &mut files);
        for f in files.iter().filter(|f| is_image_file(&f.to_string_lossy())) {
            let rel = f.strip_prefix(parent).unwrap_or(f);
            // Entry names keep the OS separator, as `assetPath.Substring` does on Windows.
            let name = rel.to_string_lossy().to_string();
            zip.start_file(name, options)?;
            zip.write_all(&std::fs::read(f)?)?;
        }
    }
    zip.finish()?;
    Ok(output.to_path_buf())
}

/// `ImageEffectPackager.ExtractPackage`: extracts the images into `destination`
/// (keeping their folders, never outside it) and returns `Config.json`'s text.
pub fn extract(package: &Path, destination: &Path) -> std::io::Result<Option<String>> {
    let file = std::fs::File::open(package)?;
    let mut zip = zip::ZipArchive::new(file).map_err(std::io::Error::other)?;
    let mut total = 0u64;
    for i in 0..zip.len() {
        total += zip.by_index(i).map_err(std::io::Error::other)?.size();
    }
    if total > MAX_UNCOMPRESSED {
        return Err(std::io::Error::other(format!(
            "archive too large: {total} bytes uncompressed, {MAX_UNCOMPRESSED} allowed"
        )));
    }
    std::fs::create_dir_all(destination)?;
    let mut config = None;
    for i in 0..zip.len() {
        let mut entry = zip.by_index(i).map_err(std::io::Error::other)?;
        let full_name = entry.name().replace('\\', "/");
        let name = full_name
            .rsplit('/')
            .next()
            .unwrap_or(&full_name)
            .to_string();
        if is_image_file(&name) {
            let rel = Path::new(&full_name);
            if rel.is_absolute() || rel.components().any(|c| !matches!(c, Component::Normal(_))) {
                continue;
            }
            let target = destination.join(rel);
            if let Some(parent) = target.parent() {
                std::fs::create_dir_all(parent)?;
            }
            let mut data = Vec::new();
            entry.read_to_end(&mut data)?;
            std::fs::write(&target, data)?;
        } else if config.is_none() && full_name.eq_ignore_ascii_case(CONFIG_FILE_NAME) {
            let mut text = String::new();
            entry.read_to_string(&mut text)?;
            config = Some(text.trim_start_matches('\u{feff}').to_string());
        }
    }
    Ok(config)
}
