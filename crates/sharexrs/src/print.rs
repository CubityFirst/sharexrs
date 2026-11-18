//! Printing an image: `TaskHelpers.PrintImage` → the Print window
//! (`ShareX.HelpersLib.PrintWindow`, unless `DontShowPrintSettingsDialog`)
//! → `PrintHelper`: Windows' print dialog (`WindowsPrintDialog`, when
//! `ShowPrintDialog`), the printer (`DefaultPrinterOverride`), and each
//! page rendered by `RenderImagePage` (the image on a white page three
//! times the page's size in hundredths of an inch, rotated to the page's
//! orientation, scaled down to fit, optionally centred) and drawn with
//! `StretchDIBits` over the page (`WindowsPrintInterop.DrawImage`).
//! Printing needs Windows; elsewhere ShareX throws, ShareXrs logs.

use image::RgbaImage;
use serde_json::Value;
use sharex_skia::raster::{Bitmap, Canvas, Paint, Rect as RRect, Shader};

/// `ShareX.HelpersLib.PrintSettings` (ApplicationConfig's `PrintSettings`).
#[derive(Clone, Debug, PartialEq)]
pub struct PrintSettings {
    pub margin: i32,
    pub auto_rotate: bool,
    pub auto_scale: bool,
    pub allow_enlarge: bool,
    pub center: bool,
    pub show_print_dialog: bool,
    pub default_printer_override: String,
}

impl Default for PrintSettings {
    fn default() -> Self {
        Self { margin: 5, auto_rotate: true, auto_scale: true, allow_enlarge: false, center: false, show_print_dialog: true, default_printer_override: String::new() }
    }
}

impl PrintSettings {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        let b = |k: &str, def: bool| v.get(k).and_then(Value::as_bool).unwrap_or(def);
        Self {
            margin: v.get("Margin").and_then(Value::as_i64).map_or(d.margin, |m| m as i32),
            auto_rotate: b("AutoRotateImage", d.auto_rotate),
            auto_scale: b("AutoScaleImage", d.auto_scale),
            allow_enlarge: b("AllowEnlargeImage", d.allow_enlarge),
            center: b("CenterImage", d.center),
            show_print_dialog: b("ShowPrintDialog", d.show_print_dialog),
            default_printer_override: v.get("DefaultPrinterOverride").and_then(Value::as_str).unwrap_or_default().to_string(),
        }
    }

    /// The fields the Print window edits, written back (the rest kept).
    pub fn write(&self, v: &mut Value) {
        if !v.is_object() {
            *v = Value::Object(Default::default());
        }
        v["Margin"] = Value::from(self.margin);
        v["AutoRotateImage"] = Value::from(self.auto_rotate);
        v["AutoScaleImage"] = Value::from(self.auto_scale);
        v["AllowEnlargeImage"] = Value::from(self.allow_enlarge);
        v["CenterImage"] = Value::from(self.center);
    }
}

/// `RotateFlipInPlace(bitmap, 1)`: a quarter turn clockwise, drawn as
/// ShareX draws it (about the centres, nearest).
fn rotate_90(src: &Bitmap) -> Bitmap {
    let (w, h) = (src.width(), src.height());
    let mut out = Bitmap::new(h, w);
    {
        let mut c = Canvas::new(&mut out);
        c.translate(h as f32 / 2.0, w as f32 / 2.0);
        c.rotate(90.0);
        c.translate(-(w as f32) / 2.0, -(h as f32) / 2.0);
        c.draw_bitmap(src, 0.0, 0.0, None);
    }
    out
}

/// `PrintHelper.RenderImagePage(source, pageSize, settings)`: the page,
/// `page` (hundredths of an inch) times three, opaque.
pub fn render_image_page(source: &RgbaImage, page: (i32, i32), s: &PrintSettings) -> RgbaImage {
    let (pw, ph) = (page.0.max(0) as u32 * 3, page.1.max(0) as u32 * 3);
    let mut out = Bitmap::new(pw.max(1), ph.max(1));
    Canvas::new(&mut out).clear([255, 255, 255, 255]);
    // Rectangle.Inflate(-margin, -margin).
    let (rx, ry, rw, rh) = (s.margin, s.margin, page.0 - 2 * s.margin, page.1 - 2 * s.margin);
    if rw > 0 && rh > 0 && source.width() > 0 && source.height() > 0 {
        let mut image = Bitmap::from_straight(source.width(), source.height(), source.as_raw());
        let (iw, ih) = (image.width() as i32, image.height() as i32);
        if s.auto_rotate && ((rw > rh && iw < ih) || (rw < rh && iw > ih)) {
            image = rotate_90(&image);
        }
        // Drawn on a copy, then only the clip (`ClipRect(rectangle)`, x3)
        // taken back: the rest of the page stays white.
        let mut drawn = out.clone();
        {
            let mut c = Canvas::new(&mut drawn);
            c.scale(3.0, 3.0);
            if s.auto_scale {
                let (iw, ih) = (image.width() as f32, image.height() as f32);
                let mut scale = (rw as f32 / iw).min(rh as f32 / ih);
                if !s.allow_enlarge {
                    scale = scale.min(1.0);
                }
                let (w, h) = (iw * scale, ih * scale);
                let (mut x, mut y) = (rx as f32, ry as f32);
                if s.center {
                    x += (rw as f32 - w) / 2.0;
                    y += (rh as f32 - h) / 2.0;
                }
                // SkiaImageHelpers.DrawImage (the canvas is scaled: Mitchell).
                let src = RRect::xywh(0.0, 0.0, iw, ih);
                c.draw_image_sx(&image, src, RRect::xywh(x, y, w, h), None);
            } else {
                let paint = Paint::shader(Shader::Color([0, 0, 0, 255]));
                c.draw_bitmap(&image, rx as f32, ry as f32, Some(&paint));
            }
        }
        let (cx0, cy0, cx1, cy1) = ((rx * 3).max(0) as u32, (ry * 3).max(0) as u32, ((rx + rw) * 3).min(pw as i32) as u32, ((ry + rh) * 3).min(ph as i32) as u32);
        for y in cy0..cy1 {
            for x in cx0..cx1 {
                out.set_pixel(x, y, drawn.pixel(x, y));
            }
        }
    }
    RgbaImage::from_raw(out.width(), out.height(), out.to_straight()).unwrap_or_else(|| RgbaImage::new(1, 1))
}

/// The default printer's page (`DefaultPageSettings.Bounds.Size`, in
/// hundredths of an inch), or Letter (`RenderPreviewPage`'s fallback).
pub fn default_page_size(s: &PrintSettings) -> (i32, i32) {
    #[cfg(windows)]
    if let Some(size) = win::page_size(&win::printer_name(s)) {
        return size;
    }
    let _ = s;
    (850, 1100)
}

/// `PrintHelper.Print`: the dialog (when `ShowPrintDialog`), then the
/// page on the chosen printer. `Ok(false)` when the dialog is cancelled.
pub fn print_image(image: &RgbaImage, s: &PrintSettings, owner: isize) -> Result<bool, String> {
    #[cfg(windows)]
    {
        win::print(image, s, owner)
    }
    #[cfg(not(windows))]
    {
        let _ = (image, s, owner);
        Err("A printing backend is required for this platform.".into())
    }
}

#[cfg(windows)]
mod win {
    use super::{PrintSettings, render_image_page};
    use image::RgbaImage;
    use windows_sys::Win32::Foundation::GlobalFree;
    use windows_sys::Win32::Graphics::Gdi::{
        BITMAPINFO, BITMAPINFOHEADER, CreateDCW, DIB_RGB_COLORS, DeleteDC, GetDeviceCaps, HDC, LOGPIXELSX, LOGPIXELSY, PHYSICALHEIGHT,
        PHYSICALWIDTH, SRCCOPY, StretchDIBits,
    };
    use windows_sys::Win32::Graphics::Printing::GetDefaultPrinterW;
    use windows_sys::Win32::Storage::Xps::{DOCINFOW, EndDoc, EndPage, StartDocW, StartPage};
    use windows_sys::Win32::UI::Controls::Dialogs::{PD_RESULT_PRINT, PRINTDLGEXW, PrintDlgExW};

    fn wide(s: &str) -> Vec<u16> {
        s.encode_utf16().chain(std::iter::once(0)).collect()
    }

    /// `PrinterSettings.PrinterName`: the override, else the default.
    pub fn printer_name(s: &PrintSettings) -> String {
        if !s.default_printer_override.is_empty() {
            return s.default_printer_override.clone();
        }
        let mut len = 0u32;
        // SAFETY: asks for the length, then fills a buffer of that length.
        unsafe {
            GetDefaultPrinterW(std::ptr::null_mut(), &mut len);
            let mut buf = vec![0u16; len as usize + 1];
            if GetDefaultPrinterW(buf.as_mut_ptr(), &mut len) == 0 {
                return String::new();
            }
            String::from_utf16_lossy(&buf[..len.saturating_sub(1) as usize])
        }
    }

    fn printer_dc(name: &str) -> HDC {
        let (driver, device) = (wide("WINSPOOL"), wide(name));
        // SAFETY: NUL-terminated strings; no DEVMODE (the printer's default).
        unsafe { CreateDCW(driver.as_ptr(), device.as_ptr(), std::ptr::null(), std::ptr::null()) }
    }

    /// The page's bounds in hundredths of an inch.
    fn dc_page(dc: HDC) -> (i32, i32) {
        // SAFETY: a valid printer DC.
        unsafe {
            let (pw, ph) = (GetDeviceCaps(dc, PHYSICALWIDTH as i32), GetDeviceCaps(dc, PHYSICALHEIGHT as i32));
            let (dx, dy) = (GetDeviceCaps(dc, LOGPIXELSX as i32).max(1), GetDeviceCaps(dc, LOGPIXELSY as i32).max(1));
            (((pw as f64) * 100.0 / dx as f64).round() as i32, ((ph as f64) * 100.0 / dy as f64).round() as i32)
        }
    }

    pub fn page_size(name: &str) -> Option<(i32, i32)> {
        if name.is_empty() {
            return None;
        }
        let dc = printer_dc(name);
        if dc.is_null() {
            return None;
        }
        let size = dc_page(dc);
        // SAFETY: created above.
        unsafe { DeleteDC(dc) };
        Some(size)
    }

    /// `WindowsPrintDialog.Show`: `PrintDlgEx` returning a DC.
    fn dialog(owner: isize) -> Result<Option<HDC>, String> {
        // SAFETY: a zeroed PRINTDLGEXW with its size and flags set, the
        // handles it returns freed.
        unsafe {
            let mut d: PRINTDLGEXW = std::mem::zeroed();
            d.lStructSize = std::mem::size_of::<PRINTDLGEXW>() as u32;
            d.hwndOwner = owner as _;
            // PD_RETURNDC | PD_NOSELECTION | PD_NOPAGENUMS | PD_USEDEVMODECOPIESANDCOLLATE | PD_NOCURRENTPAGE
            d.Flags = 0x100 | 0x4 | 0x8 | 0x40000 | 0x800000;
            d.nMinPage = 1;
            d.nMaxPage = 9999;
            d.nCopies = 1;
            d.nStartPage = u32::MAX; // START_PAGE_GENERAL
            let hr = PrintDlgExW(&mut d);
            if !d.hDevMode.is_null() {
                GlobalFree(d.hDevMode);
            }
            if !d.hDevNames.is_null() {
                GlobalFree(d.hDevNames);
            }
            if hr < 0 {
                return Err(format!("PrintDlgEx failed: 0x{:08X}", hr as u32));
            }
            if d.dwResultAction != PD_RESULT_PRINT {
                if !d.hDC.is_null() {
                    DeleteDC(d.hDC);
                }
                return Ok(None);
            }
            Ok(Some(d.hDC))
        }
    }

    pub fn print(image: &RgbaImage, s: &PrintSettings, owner: isize) -> Result<bool, String> {
        let dc = if s.show_print_dialog {
            match dialog(owner)? {
                Some(dc) => dc,
                None => return Ok(false),
            }
        } else {
            // `TryDefaultPrinterOverride`: an invalid override falls back
            // to the default printer.
            let mut dc = printer_dc(&printer_name(s));
            if dc.is_null() && !s.default_printer_override.is_empty() {
                eprintln!("Print: invalid printer \"{}\"; using the default printer", s.default_printer_override);
                dc = printer_dc(&printer_name(&PrintSettings { default_printer_override: String::new(), ..s.clone() }));
            }
            dc
        };
        if dc.is_null() {
            return Err("no printer".into());
        }
        let page = dc_page(dc);
        let rendered = render_image_page(image, page, s);
        let result = draw_page(dc, &rendered, page);
        // SAFETY: the DC is ours.
        unsafe { DeleteDC(dc) };
        result.map(|()| true)
    }

    /// One document of one page: `WindowsPrintInterop.DrawImage` over the
    /// page bounds at the printer's resolution (24-bit DIB, bottom-up).
    fn draw_page(dc: HDC, page_img: &RgbaImage, page: (i32, i32)) -> Result<(), String> {
        let (w, h) = (page_img.width() as i32, page_img.height() as i32);
        let stride = ((w * 3 + 3) / 4 * 4) as usize;
        let mut bits = vec![0u8; stride * h as usize];
        for y in 0..h {
            let row = &mut bits[(h - 1 - y) as usize * stride..][..stride];
            for x in 0..w {
                let p = page_img.get_pixel(x as u32, y as u32).0;
                row[x as usize * 3..x as usize * 3 + 3].copy_from_slice(&[p[2], p[1], p[0]]);
            }
        }
        let name = wide("ShareXrs");
        // Tests print to a file (no printer driver dialog).
        let output = std::env::var("SHAREXRS_PRINT_TO_FILE").ok().map(|p| wide(&p));
        // SAFETY: the DOCINFO strings and the DIB outlive the calls.
        unsafe {
            let mut doc: DOCINFOW = std::mem::zeroed();
            doc.cbSize = std::mem::size_of::<DOCINFOW>() as i32;
            doc.lpszDocName = name.as_ptr();
            if let Some(o) = &output {
                doc.lpszOutput = o.as_ptr();
            }
            if StartDocW(dc, &doc) <= 0 {
                return Err("StartDoc failed".into());
            }
            StartPage(dc);
            let (dx, dy) = (GetDeviceCaps(dc, LOGPIXELSX as i32) as f32 / 100.0, GetDeviceCaps(dc, LOGPIXELSY as i32) as f32 / 100.0);
            let (rw, rh) = ((page.0 as f32 * dx).round() as i32, (page.1 as f32 * dy).round() as i32);
            let mut info: BITMAPINFO = std::mem::zeroed();
            info.bmiHeader = BITMAPINFOHEADER {
                biSize: 40,
                biWidth: w,
                biHeight: h,
                biPlanes: 1,
                biBitCount: 24,
                biCompression: 0,
                biSizeImage: bits.len() as u32,
                biXPelsPerMeter: 0,
                biYPelsPerMeter: 0,
                biClrUsed: 0,
                biClrImportant: 0,
            };
            let r = StretchDIBits(dc, 0, 0, rw, rh, 0, 0, w, h, bits.as_ptr().cast(), &info, DIB_RGB_COLORS, SRCCOPY);
            EndPage(dc);
            EndDoc(dc);
            if r <= 0 {
                return Err("The image could not be drawn on the printer page (StretchDIBits failed).".into());
            }
        }
        Ok(())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn page_is_three_times_and_white() {
        let img = RgbaImage::from_pixel(40, 20, image::Rgba([10, 20, 30, 255]));
        let page = render_image_page(&img, (100, 50), &PrintSettings::default());
        assert_eq!((page.width(), page.height()), (300, 150));
        assert_eq!(page.get_pixel(0, 0).0, [255, 255, 255, 255]);
        assert_eq!(page.get_pixel(20, 20).0, [10, 20, 30, 255]);
    }

    /// `RenderImagePage` against ShareX's own (tools/tools-harness
    /// `--print-page`, tests/data/print).
    #[test]
    fn pages_are_sharexs() {
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/print");
        let img = image::open(dir.join("image.png")).unwrap().to_rgba8();
        let s = |m: i32, rot: bool, scale: bool, big: bool, center: bool| PrintSettings {
            margin: m,
            auto_rotate: rot,
            auto_scale: scale,
            allow_enlarge: big,
            center,
            ..PrintSettings::default()
        };
        let cases = [
            ("rotate-fit", (300, 400), s(5, true, true, false, false)),
            ("enlarge-center", (900, 300), s(10, true, true, true, true)),
            ("unscaled-clip", (300, 200), s(3, false, false, false, false)),
            ("odd-center", (213, 157), s(7, false, true, false, true)),
        ];
        let mut failures = Vec::new();
        for (name, page, settings) in cases {
            let got = render_image_page(&img, page, &settings);
            let want = image::open(dir.join(format!("{name}.png"))).unwrap().to_rgba8();
            assert_eq!(got.dimensions(), want.dimensions(), "{name}");
            let diff = got.pixels().zip(want.pixels()).filter(|(a, b)| a != b).count();
            let max = got.as_raw().iter().zip(want.as_raw()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
            // Enlarged (Mitchell upscaling): one level on a few pixels.
            let tolerance = if name == "enlarge-center" { 1 } else { 0 };
            if max > tolerance || (tolerance == 0 && diff > 0) {
                failures.push(format!("{name}: {diff} pixels differ, by up to {max}"));
            }
        }
        assert!(failures.is_empty(), "{failures:#?}");
    }

    /// Prints tests/data/print/image.png with the default settings, no
    /// dialog, to the file `SHAREXRS_PRINT_TO_FILE` on the default printer
    /// (run by hand with Microsoft Print to PDF as the default printer).
    #[test]
    #[ignore]
    #[cfg(windows)]
    fn print_to_file() {
        let path = std::env::var("SHAREXRS_PRINT_TO_FILE").expect("SHAREXRS_PRINT_TO_FILE");
        let dir = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/data/print");
        let img = image::open(dir.join("image.png")).unwrap().to_rgba8();
        let s = PrintSettings { show_print_dialog: false, ..PrintSettings::default() };
        assert!(print_image(&img, &s, 0).unwrap());
        println!("page {:?} -> {path}", default_page_size(&s));
    }
}
