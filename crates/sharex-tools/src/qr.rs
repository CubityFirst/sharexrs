//! QR code: `TaskHelpers.GenerateQRCode` / `BarcodeScan` / `SaveQRCodeAsync`
//! (ShareX on ZXing.Net 0.16.11) and the content rules of `QRCodeViewModel`,
//! on rxing (a Rust port of the same ZXing).

use rxing::{BarcodeFormat, EncodeHints, Writer};

use crate::imaging::{self, Bitmap};

/// `QRCodeViewModel.MaximumContentBytes` / `CheckQRCodeContent`.
pub const MAXIMUM_CONTENT_BYTES: usize = 2952;

/// The smallest size the window generates (`EffectiveSize`).
pub const MINIMUM_SIZE: i32 = 64;

/// `TaskHelpers.CheckQRCodeContent`: non-empty and at most 2952 UTF-8 bytes.
pub fn check_content(text: &str) -> bool {
    !text.is_empty() && text.len() <= MAXIMUM_CONTENT_BYTES
}

/// `QRCodeViewModel.EffectiveSize`: the chosen size, or the space available,
/// at least 64.
pub fn effective_size(chosen: i32, available: i32) -> i32 {
    (if chosen > 0 { chosen } else { available }).max(MINIMUM_SIZE)
}

fn hints(margin: Option<&str>) -> EncodeHints {
    EncodeHints { CharacterSet: Some("UTF-8".into()), Margin: margin.map(str::to_string), ..Default::default() }
}

fn matrix(text: &str, size: i32, margin: Option<&str>) -> Option<(u32, u32, Vec<bool>)> {
    let m = rxing::qrcode::QRCodeWriter
        .encode_with_hints(text, &BarcodeFormat::QR_CODE, size, size, &hints(margin))
        .ok()?;
    let (w, h) = (m.getWidth(), m.getHeight());
    let mut bits = Vec::with_capacity((w * h) as usize);
    for y in 0..h {
        for x in 0..w {
            bits.push(m.get(x, y));
        }
    }
    Some((w, h, bits))
}

/// The QR code's modules at `size` (ZXing's `QRCodeWriter` with ShareX's
/// options: UTF-8 with an ECI, error correction L, a one-module quiet zone):
/// width, height and the dark modules.
pub fn encode_matrix(text: &str, size: i32) -> Option<(u32, u32, Vec<bool>)> {
    if !check_content(text) {
        return None;
    }
    matrix(text, size, Some("1"))
}

/// `TaskHelpers.GenerateQRCode(text, size)`: white with a black pixel per dark
/// module pixel (`DrawRect(x, y, 1, 1)`, no anti-aliasing).
pub fn generate(text: &str, size: i32) -> Option<Bitmap> {
    let (w, h, bits) = encode_matrix(text, size)?;
    let mut bmp = imaging::create_bitmap(w, h);
    for (i, dark) in bits.iter().enumerate() {
        bmp.set_px(i, if *dark { [0, 0, 0, 255] } else { [255, 255, 255, 255] });
    }
    Some(bmp)
}

/// `GenerateQRCodePreviewAsync`: the PNG the window shows.
pub fn generate_png(text: &str, size: i32) -> Option<Vec<u8>> {
    generate(text, size).map(|b| imaging::encode_png(&b))
}

/// `SaveQRCodeAsync` to a `.svg` file: `BarcodeWriterSvg` (UTF-8, ZXing's
/// default four-module quiet zone, not the one-module margin of the PNG),
/// written with a BOM (`File.WriteAllText(path, svg, Encoding.UTF8)`).
pub fn generate_svg_file(text: &str, size: i32) -> Option<Vec<u8>> {
    let svg = generate_svg(text, size)?;
    let mut out = vec![0xEF, 0xBB, 0xBF];
    out.extend_from_slice(svg.as_bytes());
    Some(out)
}

/// The SVG `BarcodeWriterSvg` renders for `text` at `size`.
pub fn generate_svg(text: &str, size: i32) -> Option<String> {
    if text.is_empty() {
        return None;
    }
    let (w, h, bits) = matrix(text, size, None)?;
    Some(svg(w, h, &bits))
}

/// ZXing.Net's `SvgRenderer` with its defaults (black on white, no text for
/// QR codes): header, DOCTYPE, the `svg` tag, then `AppendDarkCell`'s
/// rectangles (each column's dark runs, widened to the right while every
/// row of the run stays dark).
fn svg(w: u32, h: u32, bits: &[bool]) -> String {
    let (w, h) = (w as usize, h as usize);
    let dark = |x: usize, y: usize| bits[y * w + x];
    let mut s = String::new();
    s.push_str("<?xml version=\"1.0\" standalone=\"no\"?>");
    s.push_str("<!-- Created with ZXing.Net (http://zxingnet.codeplex.com/) -->");
    s.push_str("<!DOCTYPE svg PUBLIC \"-//W3C//DTD SVG 1.1//EN\" \"http://www.w3.org/Graphics/SVG/1.1/DTD/svg11.dtd\">");
    s.push_str(&format!(
        "<svg xmlns=\"http://www.w3.org/2000/svg\" version=\"1.2\" baseProfile=\"tiny\" shape-rendering=\"crispEdges\" viewBox=\"0 0 {w} {h}\" viewport-fill=\"rgb(255,255,255)\" viewport-fill-opacity=\"1\" fill=\"rgb(0,0,0)\" fill-opacity=\"1\" style=\"background-color:rgb(255,255,255);background-color:rgba(255, 255, 255, 1);\">"
    ));
    let mut processed = vec![false; w * h];
    // FindMaximumRectangle: the last column the run [y0, y1) extends to.
    let find_max = |processed: &mut Vec<bool>, x0: usize, y0: usize, y1: usize| -> usize {
        let mut end = x0;
        for x in x0 + 1..w {
            if (y0..y1).any(|y| !dark(x, y)) {
                return end;
            }
            end = x;
            for y in y0..y1 {
                processed[y * w + x] = true;
            }
        }
        end
    };
    let rect = |s: &mut String, x: usize, y: usize, rw: usize, rh: usize| {
        s.push_str(&format!("<rect x=\"{x}\" y=\"{y}\" width=\"{rw}\" height=\"{rh}\"/>"));
    };
    for x in 0..w {
        let mut open = false;
        let (mut sx, mut sy) = (0, 0);
        for y in 0..h {
            if processed[y * w + x] {
                continue;
            }
            processed[y * w + x] = true;
            if dark(x, y) {
                if !open {
                    sx = x;
                    sy = y;
                    open = true;
                }
            } else if open {
                let end = find_max(&mut processed, sx, sy, y);
                rect(&mut s, sx, sy, end - sx + 1, y - sy);
                open = false;
            }
        }
        if open {
            let end = find_max(&mut processed, sx, sy, h);
            rect(&mut s, sx, sy, end - sx + 1, h - sy);
        }
    }
    s.push_str("</svg>");
    s
}

/// ZXing.Net's `RGBLuminanceSource` for BGRA32: weighted luminance, blended
/// toward white by the alpha.
fn luminance(bmp: &Bitmap) -> Vec<u8> {
    imaging::straight_pixels(bmp)
        .into_iter()
        .map(|p| {
            let (r, g, b, a) = (p[0] as u32, p[1] as u32, p[2] as u32, p[3] as u32);
            let l = (19562 * r + 38550 * g + 7424 * b) >> 16;
            (((l * a) >> 8) + ((255 * (255 - a)) >> 8)) as u8
        })
        .collect()
}

fn decode_multiple(luma: &[u8], w: u32, h: u32, qr_only: bool) -> Option<Vec<String>> {
    use rxing::multi::MultipleBarcodeReader;
    // Options.TryHarder and TryInverted (the ALSO_INVERTED hint, which only
    // MultiFormatReader honours).
    let mut hints = rxing::DecodeHints { TryHarder: Some(true), AlsoInverted: Some(true), ..Default::default() };
    let results = if qr_only {
        hints.PossibleFormats = Some([BarcodeFormat::QR_CODE].into_iter().collect());
        let source = rxing::Luma8LuminanceSource::new(luma.to_vec(), w, h).ok()?;
        let mut bitmap = rxing::BinaryBitmap::new(rxing::common::HybridBinarizer::new(source));
        match rxing::multi::qrcode::QRCodeMultiReader::new().decode_multiple_with_hints(&mut bitmap, &hints) {
            Ok(r) => r,
            // rxing's QRCodeMultiReader gives up when one candidate fails to
            // decode, where ZXing skips it: look again with the generic
            // multiple reader, QR codes only and (like QRCodeMultiReader) not inverted.
            Err(_) => {
                hints.AlsoInverted = None;
                rxing::helpers::detect_multiple_in_luma_with_hints(luma.to_vec(), w, h, &mut hints).ok()?
            }
        }
    } else {
        rxing::helpers::detect_multiple_in_luma_with_hints(luma.to_vec(), w, h, &mut hints).ok()?
    };
    (!results.is_empty()).then(|| results.iter().map(|r| r.getText().to_string()).collect())
}

/// `TaskHelpers.BarcodeScan(bitmap, scanQRCodeOnly)` through ZXing.Net's
/// `BarcodeReaderGeneric.DecodeMultiple`: `QRCodeMultiReader` when only QR
/// codes are wanted, else `GenericMultipleBarcodeReader` over
/// `MultiFormatReader`; with `AutoRotate`, the source turned a quarter
/// counter-clockwise when nothing is found (ZXing.Net turns the original each
/// time, so that one turn is all its four attempts try). The non-empty texts,
/// or `None`.
pub fn scan(bmp: &Bitmap, qr_only: bool) -> Option<Vec<String>> {
    let (w, h) = (bmp.width(), bmp.height());
    let luma = luminance(bmp);
    let found = decode_multiple(&luma, w, h, qr_only).or_else(|| {
        let mut rotated = vec![0u8; luma.len()];
        for y in 0..h {
            for x in 0..w {
                rotated[((w - 1 - x) * h + y) as usize] = luma[(y * w + x) as usize];
            }
        }
        decode_multiple(&rotated, h, w, qr_only)
    })?;
    Some(found.into_iter().filter(|t| !t.is_empty()).collect())
}

/// `QRCodeViewModel.ApplyScanResultsAsync`: the texts joined by a blank line.
pub fn join_results(results: &[String]) -> String {
    let sep = format!("{0}{0}", crate::dotnet::NEW_LINE);
    results.join(&sep)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn round_trip() {
        let bmp = generate("https://getsharex.com", 200).unwrap();
        assert!(bmp.width() >= 200);
        assert_eq!(scan(&bmp, false), Some(vec!["https://getsharex.com".to_string()]));
        assert!(!check_content(""));
        assert!(!check_content(&"é".repeat(1477)));
    }
}
