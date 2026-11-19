//! `ShareX.ImageEditor/Integration`: the services the editor asks its host
//! for. So far the desktop wallpaper (`IDesktopWallpaperService` and
//! ShareX's macOS and Linux implementations; Windows' lives in the app).

pub mod wallpaper;
