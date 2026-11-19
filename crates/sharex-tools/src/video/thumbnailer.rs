//! Video thumbnailer: `VideoThumbnailer` and `VideoThumbnailOptions`
//! (`TaskSettingsTools.VideoThumbnailOptions`): the frames FFmpeg extracts and
//! the combined contact sheet ShareX draws.
//!
//! The sheet's text (the video summary and each frame's timestamp, Arial)
//! goes through `sharex_image_effects::text`, which fills glyph outlines where
//! Skia rasterises DirectWrite glyph masks: glyph edges differ, as for the
//! Text watermark effect; everything else matches.

use serde_json::Value;
use sharex_image_effects::dotnet::{ImageFont, font_style};
use sharex_image_effects::ops::{draw_rectangle_proper, stroke_paint};
use sharex_image_effects::text::{Fonts, draw_text_path, measure_text};
use sharex_screen_recording::devices::VideoInfo;
use sharex_skia::raster::{Canvas, Paint, Rect, Shader};

use crate::dotnet;
use crate::imaging::{self, Bitmap, SamplingMode};
use crate::options::{enum_name, json_bool, json_i32, json_string};

/// `ThumbnailLocationType`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OutputLocation {
    DefaultFolder,
    ParentFolder,
    CustomFolder,
}

/// `EImageFormat` as the thumbnailer saves it.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ImageFormat {
    Png,
    Jpeg,
    Webp,
    Gif,
    Bmp,
}

impl ImageFormat {
    const NAMES: [&'static str; 5] = ["PNG", "JPEG", "WEBP", "GIF", "BMP"];

    /// `EImageFormat.GetFileExtension`.
    pub fn extension(self) -> &'static str {
        match self {
            ImageFormat::Jpeg => "jpg",
            ImageFormat::Webp => "webp",
            ImageFormat::Gif => "gif",
            ImageFormat::Bmp => "bmp",
            ImageFormat::Png => "png",
        }
    }
}

/// `VideoThumbnailOptions`.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoThumbnailOptions {
    pub output_location: OutputLocation,
    pub custom_output_directory: String,
    pub image_format: ImageFormat,
    pub thumbnail_count: i32,
    pub filename_suffix: String,
    pub random_frame: bool,
    pub upload_thumbnails: bool,
    pub keep_screenshots: bool,
    pub open_directory: bool,
    pub max_thumbnail_width: i32,
    pub combine_screenshots: bool,
    pub padding: i32,
    pub spacing: i32,
    pub column_count: i32,
    pub add_video_info: bool,
    pub add_timestamp: bool,
    pub draw_shadow: bool,
    pub draw_border: bool,
    pub default_output_directory: Option<String>,
    pub last_video_path: Option<String>,
}

impl Default for VideoThumbnailOptions {
    fn default() -> Self {
        VideoThumbnailOptions {
            output_location: OutputLocation::DefaultFolder,
            custom_output_directory: String::new(),
            image_format: ImageFormat::Png,
            thumbnail_count: 9,
            filename_suffix: "_Thumbnail".into(),
            random_frame: false,
            upload_thumbnails: true,
            keep_screenshots: false,
            open_directory: false,
            max_thumbnail_width: 512,
            combine_screenshots: true,
            padding: 10,
            spacing: 10,
            column_count: 3,
            add_video_info: true,
            add_timestamp: true,
            draw_shadow: true,
            draw_border: true,
            default_output_directory: None,
            last_video_path: None,
        }
    }
}

impl VideoThumbnailOptions {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let b = |k: &str, def: bool| json_bool(&v[k]).unwrap_or(def);
        let i = |k: &str, def: i32| json_i32(&v[k]).unwrap_or(def);
        let s = |k: &str, def: &str| json_string(&v[k]).flatten().unwrap_or_else(|| def.to_string());
        VideoThumbnailOptions {
            output_location: match enum_name(&v["OutputLocation"], &["DefaultFolder", "ParentFolder", "CustomFolder"]) {
                Some(1) => OutputLocation::ParentFolder,
                Some(2) => OutputLocation::CustomFolder,
                Some(_) => OutputLocation::DefaultFolder,
                None => d.output_location,
            },
            custom_output_directory: s("CustomOutputDirectory", &d.custom_output_directory),
            image_format: enum_name(&v["ImageFormat"], &ImageFormat::NAMES)
                .map(|n| [ImageFormat::Png, ImageFormat::Jpeg, ImageFormat::Webp, ImageFormat::Gif, ImageFormat::Bmp][n])
                .unwrap_or(d.image_format),
            thumbnail_count: i("ThumbnailCount", d.thumbnail_count),
            filename_suffix: s("FilenameSuffix", &d.filename_suffix),
            random_frame: b("RandomFrame", d.random_frame),
            upload_thumbnails: b("UploadThumbnails", d.upload_thumbnails),
            keep_screenshots: b("KeepScreenshots", d.keep_screenshots),
            open_directory: b("OpenDirectory", d.open_directory),
            max_thumbnail_width: i("MaxThumbnailWidth", d.max_thumbnail_width),
            combine_screenshots: b("CombineScreenshots", d.combine_screenshots),
            padding: i("Padding", d.padding),
            spacing: i("Spacing", d.spacing),
            column_count: i("ColumnCount", d.column_count),
            add_video_info: b("AddVideoInfo", d.add_video_info),
            add_timestamp: b("AddTimestamp", d.add_timestamp),
            draw_shadow: b("DrawShadow", d.draw_shadow),
            draw_border: b("DrawBorder", d.draw_border),
            default_output_directory: json_string(&v["DefaultOutputDirectory"]).flatten(),
            last_video_path: json_string(&v["LastVideoPath"]).flatten(),
        }
    }

    /// As Json.NET writes it (the public fields first, then the properties).
    pub fn to_json(&self) -> Value {
        let location = ["DefaultFolder", "ParentFolder", "CustomFolder"][self.output_location as usize];
        serde_json::json!({
            "DefaultOutputDirectory": self.default_output_directory,
            "LastVideoPath": self.last_video_path,
            "OutputLocation": location,
            "CustomOutputDirectory": self.custom_output_directory,
            "ImageFormat": ImageFormat::NAMES[self.image_format as usize],
            "ThumbnailCount": self.thumbnail_count,
            "FilenameSuffix": self.filename_suffix,
            "RandomFrame": self.random_frame,
            "UploadThumbnails": self.upload_thumbnails,
            "KeepScreenshots": self.keep_screenshots,
            "OpenDirectory": self.open_directory,
            "MaxThumbnailWidth": self.max_thumbnail_width,
            "CombineScreenshots": self.combine_screenshots,
            "Padding": self.padding,
            "Spacing": self.spacing,
            "ColumnCount": self.column_count,
            "AddVideoInfo": self.add_video_info,
            "AddTimestamp": self.add_timestamp,
            "DrawShadow": self.draw_shadow,
            "DrawBorder": self.draw_border,
        })
    }

    /// `GetOutputDirectory` (without creating it).
    pub fn output_directory(&self, media_path: &str) -> String {
        match self.output_location {
            OutputLocation::DefaultFolder => self.default_output_directory.clone().unwrap_or_default(),
            OutputLocation::ParentFolder => dotnet::directory_name(media_path).unwrap_or_default(),
            OutputLocation::CustomFolder => self.custom_output_directory.clone(),
        }
    }
}

/// A taken frame: its file and the second it was taken at.
#[derive(Debug, Clone, PartialEq)]
pub struct VideoThumbnailInfo {
    pub file_path: String,
    pub timestamp_seconds: i64,
}

/// `GetTimeSlice(count)`: whole seconds per slice.
pub fn time_slice(duration_seconds: f64, count: i32) -> i32 {
    (duration_seconds / count as f64) as i32
}

/// `GetRandomTimeSlice(start)` with `RandomFast.NextDouble()`'s value.
pub fn random_time_slice(duration_seconds: f64, count: i32, start: usize, next_double: f64) -> i32 {
    let seeks: Vec<i32> = (1..count + 2).map(|i| time_slice(duration_seconds, count + 2) * i).collect();
    (next_double * (seeks[start + 1] - seeks[start]) as f64 + seeks[start] as f64) as i32
}

/// The second each thumbnail is taken at (not random).
pub fn timestamps(duration_seconds: f64, count: i32) -> Vec<i32> {
    (0..count).map(|i| time_slice(duration_seconds, count) * (i + 1)).collect()
}

/// `{name}-{second}.{ext}` in the output folder.
pub fn frame_path(output_dir: &str, media_path: &str, second: i32, format: ImageFormat) -> String {
    dotnet::combine(output_dir, &format!("{}-{second}.{}", dotnet::file_name_without_extension(media_path), format.extension()))
}

/// The FFmpeg arguments for one frame.
pub fn frame_arguments(media_path: &str, second: i32, output: &str) -> String {
    format!("-ss {second} -i \"{media_path}\" -f image2 -vframes 1 -y \"{output}\"")
}

/// The combined image's path: `{name}{suffix}.{ext}`.
pub fn combined_path(output_dir: &str, media_path: &str, o: &VideoThumbnailOptions) -> String {
    dotnet::combine(
        output_dir,
        &format!("{}{}.{}", dotnet::file_name_without_extension(media_path), o.filename_suffix, o.image_format.extension()),
    )
}

/// `TimeSpan.ToString()` for whole seconds: `[d.]hh:mm:ss`.
pub fn timespan_c(seconds: i64) -> String {
    let (d, h, m, s) = (seconds / 86400, (seconds / 3600) % 24, (seconds / 60) % 60, seconds % 60);
    if d > 0 { format!("{d}.{h:02}:{m:02}:{s:02}") } else { format!("{h:02}:{m:02}:{s:02}") }
}

/// `VideoInfo.ToString()`: the summary line at the top of the sheet.
pub fn video_info_text(v: &VideoInfo) -> String {
    let ticks = v.duration;
    let total = ticks / 10_000_000;
    // Duration.ToString(@"hh\:mm\:s")
    let duration = format!("{:02}:{:02}:{}", (total / 3600) % 24, (total / 60) % 60, total % 60);
    let mut text = crate::text::format_dotnet(
        sharex_strings::helpers_lib::VideoInfo_Summary(),
        &[dotnet::file_name(&v.file_path).into(), duration.into(), (v.bitrate as i64).into()],
    );
    if let Some(codec) = v.video_codec.as_deref().filter(|c| !c.is_empty()) {
        let (w, h) = v.video_resolution.unwrap_or((0, 0));
        text += &crate::text::format_dotnet(
            sharex_strings::helpers_lib::VideoInfo_Video_details(),
            &[
                codec.into(),
                (w as i64).into(),
                (h as i64).into(),
                crate::text::format_number(v.video_fps.unwrap_or(0.0), "0.##", false).into(),
            ],
        );
    }
    if let Some(codec) = v.audio_codec.as_deref().filter(|c| !c.is_empty()) {
        text += &crate::text::format_dotnet(sharex_strings::helpers_lib::VideoInfo_Audio_details(), &[codec.into()]);
    }
    text
}

/// `CombineScreenshots`: the frames in a grid on WhiteSmoke, with the video's
/// summary above, shadows, borders and timestamps.
pub fn combine(frames: &[(Bitmap, i64)], info: Option<&VideoInfo>, o: &VideoThumbnailOptions) -> Bitmap {
    let mut fonts = Fonts::default();
    let info_font = fonts.font(&ImageFont::new("Arial", 12.0));
    let mut stamp = ImageFont::new("Arial", 10.0);
    stamp.style = font_style::BOLD;
    let stamp_font = fonts.font(&stamp);
    let info_text = if o.add_video_info { info.map(video_info_text).unwrap_or_default() } else { String::new() };
    let info_h = if o.add_video_info { info_font.as_ref().map(|f| measure_text(f, &info_text).1).unwrap_or(0) } else { 0 };
    let images: Vec<Bitmap> = frames
        .iter()
        .map(|(b, _)| {
            if o.max_thumbnail_width > 0 && b.width() as i32 > o.max_thumbnail_width {
                let h = (o.max_thumbnail_width as f32 / b.width() as f32 * b.height() as f32) as i32;
                imaging::resize_image(b.clone(), o.max_thumbnail_width, h, SamplingMode::HighQualityBicubic)
            } else {
                b.clone()
            }
        })
        .collect();
    let cols = o.column_count;
    let (tw, th) = (images[0].width() as i32, images[0].height() as i32);
    let width = o.padding * 2 + tw * cols + (cols - 1) * o.spacing;
    let rows = (images.len() as f32 / cols as f32).ceil() as i32;
    let height = o.padding * 3 + info_h + th * rows + (rows - 1) * o.spacing;
    let mut out = imaging::create_bitmap(width.max(0) as u32, height.max(0) as u32);
    {
        let mut g = Canvas::new(&mut out);
        g.clear(imaging::WHITE_SMOKE);
        if !info_text.is_empty() {
            if let Some(path) = info_font.as_ref().and_then(|f| draw_text_path(f, &info_text, o.padding as f32, o.padding as f32)) {
                g.fill_path(&path, &Paint::fill(imaging::BLACK));
            }
        }
        let mut i = 0;
        let mut y = o.padding * 2 + info_h;
        'rows: for _ in 0..rows {
            let mut x = o.padding;
            for _ in 0..cols {
                if o.draw_shadow {
                    g.draw_rect(Rect::xywh((x + 3) as f32, (y + 3) as f32, tw as f32, th as f32), &Paint::fill([0, 0, 0, 75]));
                }
                g.draw_image_xywh(&images[i], x as f32, y as f32, tw as f32, th as f32);
                if o.draw_border {
                    let stroke = stroke_paint(Shader::Color(imaging::BLACK), 1.0);
                    draw_rectangle_proper(&mut g, &stroke, Rect::xywh(x as f32, y as f32, tw as f32, th as f32), None);
                }
                if o.add_timestamp {
                    if let Some(f) = stamp_font.as_ref() {
                        let text = timespan_c(frames[i].1);
                        for (dx, color) in [(11, imaging::BLACK), (10, imaging::WHITE)] {
                            if let Some(p) = draw_text_path(f, &text, (x + dx) as f32, (y + dx) as f32) {
                                g.fill_path(&p, &Paint::fill(color));
                            }
                        }
                    }
                }
                i += 1;
                if i >= images.len() {
                    break 'rows;
                }
                x += tw + o.spacing;
            }
            y += th + o.spacing;
        }
    }
    out
}

/// `VideoThumbnailer.TakeThumbnails`: probes the video, has FFmpeg write each
/// frame, and combines them (deleting the frames unless `KeepScreenshots`).
/// `next_double` supplies `RandomFast.NextDouble()` for random frames.
pub fn take_thumbnails(
    ffmpeg: &std::path::Path,
    media_path: &str,
    o: &VideoThumbnailOptions,
    mut next_double: impl FnMut() -> f64,
    mut progress: impl FnMut(i32, i32),
) -> Result<Option<Vec<VideoThumbnailInfo>>, String> {
    let manager = sharex_screen_recording::cli::FFmpegCliManager::new(ffmpeg);
    let Some(info) = manager.get_video_info(media_path) else { return Ok(None) };
    if info.duration == 0 {
        return Ok(None);
    }
    let seconds = info.duration as f64 / 10_000_000.0;
    let dir = o.output_directory(media_path);
    std::fs::create_dir_all(&dir).map_err(|e| e.to_string())?;
    let mut taken = Vec::new();
    for i in 0..o.thumbnail_count {
        let second = if o.random_frame {
            random_time_slice(seconds, o.thumbnail_count, i as usize, next_double())
        } else {
            time_slice(seconds, o.thumbnail_count) * (i + 1)
        };
        let path = frame_path(&dir, media_path, second, o.image_format);
        let mut child = sharex_screen_recording::cli::command(ffmpeg, &frame_arguments(media_path, second, &path))
            .stdout(std::process::Stdio::null())
            .stderr(std::process::Stdio::null())
            .spawn()
            .map_err(|e| e.to_string())?;
        let deadline = std::time::Instant::now() + std::time::Duration::from_secs(30);
        while child.try_wait().map_err(|e| e.to_string())?.is_none() && std::time::Instant::now() < deadline {
            std::thread::sleep(std::time::Duration::from_millis(20));
        }
        if std::path::Path::new(&path).exists() {
            taken.push(VideoThumbnailInfo { file_path: path, timestamp_seconds: second as i64 });
        }
        progress(i + 1, o.thumbnail_count);
    }
    if taken.is_empty() {
        return Ok(Some(Vec::new()));
    }
    if !o.combine_screenshots {
        return Ok(Some(taken));
    }
    let frames: Vec<(Bitmap, i64)> = taken
        .iter()
        .map(|t| imaging::load_image(&t.file_path).map(|b| (b, t.timestamp_seconds)).ok_or("frame"))
        .collect::<Result<_, _>>()?;
    let sheet = combine(&frames, Some(&info), o);
    let path = combined_path(&dir, media_path, o);
    imaging::save_image(&sheet, &path).map_err(|e| e.to_string())?;
    if !o.keep_screenshots {
        for t in &taken {
            let _ = std::fs::remove_file(&t.file_path);
        }
    }
    Ok(Some(vec![VideoThumbnailInfo { file_path: path, timestamp_seconds: 0 }]))
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn options_round_trip_as_sharex_writes_them() {
        let text = r#"{"DefaultOutputDirectory":null,"LastVideoPath":null,"OutputLocation":"DefaultFolder","CustomOutputDirectory":"","ImageFormat":"PNG","ThumbnailCount":9,"FilenameSuffix":"_Thumbnail","RandomFrame":false,"UploadThumbnails":true,"KeepScreenshots":false,"OpenDirectory":false,"MaxThumbnailWidth":512,"CombineScreenshots":true,"Padding":10,"Spacing":10,"ColumnCount":3,"AddVideoInfo":true,"AddTimestamp":true,"DrawShadow":true,"DrawBorder":true}"#;
        let v: Value = serde_json::from_str(text).unwrap();
        let o = VideoThumbnailOptions::from_json(&v);
        assert_eq!(o, VideoThumbnailOptions::default());
        assert_eq!(serde_json::to_string(&o.to_json()).unwrap(), text);
    }

    #[test]
    fn slices_and_names() {
        assert_eq!(timestamps(10.0, 3), vec![3, 6, 9]);
        assert_eq!(timespan_c(3725), "01:02:05");
        let sep = if cfg!(windows) { '\\' } else { '/' };
        assert_eq!(frame_path("out", "v/clip.mp4", 6, ImageFormat::Jpeg), format!("out{sep}clip-6.jpg"));
        assert_eq!(frame_arguments("a.mp4", 6, "b.png"), "-ss 6 -i \"a.mp4\" -f image2 -vframes 1 -y \"b.png\"");
    }
}
