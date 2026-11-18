//! Scrolling capture's engine: `ScrollingCaptureOptions.cs`,
//! `ScrollingCaptureManager.cs` (the capture loop, `CompareLastTwoImages`,
//! `CombineImages`) and the `ScrollingCaptureStatus`/`ScrollMethod` enums
//! (`Enums.cs`), without the windows. The loop drives a [`ScrollTarget`]
//! (the platform's screen grab and input, or a list of frames in tests).
//!
//! Validated against ShareX's own `ScrollingCaptureManager` by
//! `tools/scrolling-harness` (the frames and results are the fixtures of
//! `tests/scrolling.rs`).

use std::sync::atomic::{AtomicBool, Ordering};
use std::time::{Duration, Instant};

use image::RgbaImage;
use serde_json::{Value, json};

/// `ScrollMethod` (localized: `ScrollMethod_<name>`).
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollMethod {
    #[default]
    MouseWheel,
    DownArrow,
    PageDown,
    ScrollMessage,
}

impl ScrollMethod {
    pub const ALL: [ScrollMethod; 4] = [Self::MouseWheel, Self::DownArrow, Self::PageDown, Self::ScrollMessage];

    /// The JSON name (Json.NET's `StringEnumConverter`).
    pub fn name(self) -> &'static str {
        match self {
            Self::MouseWheel => "MouseWheel",
            Self::DownArrow => "DownArrow",
            Self::PageDown => "PageDown",
            Self::ScrollMessage => "ScrollMessage",
        }
    }

    /// From its name or number, as Json.NET reads an enum.
    pub fn parse(v: &Value) -> Option<Self> {
        if let Some(n) = v.as_i64() {
            return Self::ALL.get(usize::try_from(n).ok()?).copied();
        }
        let s = v.as_str()?;
        Self::ALL.iter().copied().find(|m| m.name().eq_ignore_ascii_case(s.trim()))
    }

    /// `Helpers.GetLocalizedEnumDescriptions<ScrollMethod>()`.
    pub fn description(self) -> &'static str {
        use sharex_strings::helpers_lib as s;
        match self {
            Self::MouseWheel => s::ScrollMethod_MouseWheel(),
            Self::DownArrow => s::ScrollMethod_DownArrow(),
            Self::PageDown => s::ScrollMethod_PageDown(),
            Self::ScrollMessage => s::ScrollMethod_ScrollMessage(),
        }
    }

    pub fn index(self) -> usize {
        Self::ALL.iter().position(|m| *m == self).unwrap_or(0)
    }
}

/// `ScrollingCaptureStatus`.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub enum ScrollingCaptureStatus {
    #[default]
    Failed,
    PartiallySuccessful,
    Successful,
}

impl ScrollingCaptureStatus {
    pub fn name(self) -> &'static str {
        match self {
            Self::Failed => "Failed",
            Self::PartiallySuccessful => "PartiallySuccessful",
            Self::Successful => "Successful",
        }
    }
}

/// `ScrollingCaptureOptions` (`TaskSettingsCapture.ScrollingCaptureOptions`).
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct ScrollingCaptureOptions {
    pub start_delay: i32,
    pub auto_scroll_top: bool,
    pub scroll_delay: i32,
    pub scroll_method: ScrollMethod,
    pub scroll_amount: i32,
    pub auto_ignore_bottom_edge: bool,
    pub auto_upload: bool,
    pub show_region: bool,
}

impl Default for ScrollingCaptureOptions {
    fn default() -> Self {
        Self {
            start_delay: 300,
            auto_scroll_top: false,
            scroll_delay: 300,
            scroll_method: ScrollMethod::MouseWheel,
            scroll_amount: 2,
            auto_ignore_bottom_edge: true,
            auto_upload: false,
            show_region: true,
        }
    }
}

fn int(v: &Value, key: &str, default: i32) -> i32 {
    v.get(key).and_then(Value::as_i64).map_or(default, |n| n.clamp(i32::MIN as i64, i32::MAX as i64) as i32)
}

fn boolean(v: &Value, key: &str, default: bool) -> bool {
    v.get(key).and_then(Value::as_bool).unwrap_or(default)
}

fn set(target: &mut Value, key: &str, value: Value) {
    if !target.is_object() {
        *target = json!({});
    }
    if let Some(m) = target.as_object_mut() {
        m.insert(key.to_string(), value);
    }
}

impl ScrollingCaptureOptions {
    pub fn from_json(v: &Value) -> Self {
        let d = Self::default();
        Self {
            start_delay: int(v, "StartDelay", d.start_delay),
            auto_scroll_top: boolean(v, "AutoScrollTop", d.auto_scroll_top),
            scroll_delay: int(v, "ScrollDelay", d.scroll_delay),
            scroll_method: v.get("ScrollMethod").and_then(ScrollMethod::parse).unwrap_or(d.scroll_method),
            scroll_amount: int(v, "ScrollAmount", d.scroll_amount),
            auto_ignore_bottom_edge: boolean(v, "AutoIgnoreBottomEdge", d.auto_ignore_bottom_edge),
            auto_upload: boolean(v, "AutoUpload", d.auto_upload),
            show_region: boolean(v, "ShowRegion", d.show_region),
        }
    }

    /// Writes every property into `target` in ShareX's order (keeping the
    /// place of those already there).
    pub fn write_json(&self, target: &mut Value) {
        set(target, "StartDelay", json!(self.start_delay));
        set(target, "AutoScrollTop", json!(self.auto_scroll_top));
        set(target, "ScrollDelay", json!(self.scroll_delay));
        set(target, "ScrollMethod", json!(self.scroll_method.name()));
        set(target, "ScrollAmount", json!(self.scroll_amount));
        set(target, "AutoIgnoreBottomEdge", json!(self.auto_ignore_bottom_edge));
        set(target, "AutoUpload", json!(self.auto_upload));
        set(target, "ShowRegion", json!(self.show_region));
    }
}

/// `SkiaImageHelpers.CompareImages`: the same size and the same pixels.
pub fn compare_images(first: &RgbaImage, second: &RgbaImage) -> bool {
    first.dimensions() == second.dimensions() && first.as_raw() == second.as_raw()
}

/// The stitching state of one capture (`ScrollingCaptureManager`'s
/// `Result`, `status` and best match fields).
#[derive(Clone, Debug, Default)]
pub struct Stitcher {
    /// `Options.AutoIgnoreBottomEdge`.
    pub auto_ignore_bottom_edge: bool,
    pub result: Option<RgbaImage>,
    pub status: ScrollingCaptureStatus,
    best_match_count: i32,
    best_match_index: i32,
    best_ignore_bottom_offset: i32,
}

impl Stitcher {
    pub fn new(auto_ignore_bottom_edge: bool) -> Self {
        Self { auto_ignore_bottom_edge, ..Self::default() }
    }

    /// `CombineImagesAsync(Result, currentImage)` and the loop's use of it:
    /// the result grows by `current` and true, or false when it can't be
    /// stitched (the capture ends; the result so far is kept).
    pub fn add(&mut self, current: &RgbaImage) -> bool {
        let result = self.result.take();
        match self.combine(result.as_ref(), current) {
            Some(r) => {
                self.result = Some(r);
                true
            }
            None => {
                self.result = result;
                false
            }
        }
    }

    /// `CombineImages(result, currentImage)`.
    pub fn combine(&mut self, result: Option<&RgbaImage>, current: &RgbaImage) -> Option<RgbaImage> {
        let Some(result) = result else {
            self.status = ScrollingCaptureStatus::Successful;
            return Some(current.clone());
        };
        let (rw, rh) = (result.width() as i32, result.height() as i32);
        let (cw, ch) = (current.width() as i32, current.height() as i32);
        // A frame of another size (the display changed mid-capture) can't
        // be stitched on: the capture ends with what it has.
        if cw != rw || ch > rh {
            self.status = ScrollingCaptureStatus::Failed;
            return None;
        }

        let mut match_count = 0;
        let mut match_index = 0;
        let match_limit = ch / 2;

        let ignore_side_offset = 50.max(cw / 20).min(cw / 3);
        let rect_width = cw - ignore_side_offset * 2;
        let rect_bottom_edge = rh; // rect.Bottom: (rh - ch) + ch
        let compare_length = (4 * rect_width.max(0)) as usize;
        let side = (ignore_side_offset.max(0) * 4) as usize;
        let stride = (rw * 4) as usize;
        let r_raw = result.as_raw();
        let c_raw = current.as_raw();
        // `CompareRows` over the compared width of row `ry` of the result
        // and row `cy` of the current image (rows outside an image never
        // match: ShareX would read out of its buffer there).
        let rows_equal = |ry: i32, cy: i32| -> bool {
            if ry < 0 || ry >= rh || cy < 0 || cy >= ch {
                return false;
            }
            let r0 = ry as usize * stride + side;
            let c0 = cy as usize * stride + side;
            r_raw[r0..r0 + compare_length] == c_raw[c0..c0 + compare_length]
        };

        let ignore_bottom_offset_max = ch / 3;
        let mut ignore_bottom_offset = 50.max(ch / 10);

        if self.auto_ignore_bottom_edge {
            for i in 0..=ignore_bottom_offset_max {
                if !rows_equal(rh - 1 - i, ch - 1 - i) {
                    ignore_bottom_offset += i;
                    break;
                }
            }
            ignore_bottom_offset = ignore_bottom_offset.max(self.best_ignore_bottom_offset);
        }

        ignore_bottom_offset = ignore_bottom_offset.min(ignore_bottom_offset_max);

        let rect_bottom = rect_bottom_edge - ignore_bottom_offset - 1;

        let mut current_y = ch - 1;
        while current_y >= 0 && match_count < match_limit {
            let mut current_match_count = 0;
            let mut y = 0;
            while current_y - y >= 0 && current_match_count < match_limit {
                if rows_equal(rect_bottom - y, current_y - y) {
                    current_match_count += 1;
                } else {
                    break;
                }
                y += 1;
            }
            if current_match_count > match_count {
                match_count = current_match_count;
                match_index = current_y;
            }
            current_y -= 1;
        }

        let mut best_guess = false;
        if match_count == 0 && self.best_match_count > 0 {
            match_count = self.best_match_count;
            match_index = self.best_match_index;
            ignore_bottom_offset = self.best_ignore_bottom_offset;
            best_guess = true;
        }

        if match_count > 0 {
            let match_height = ch - match_index - 1;
            if match_height > 0 {
                if match_count > self.best_match_count {
                    self.best_match_count = match_count;
                    self.best_match_index = match_index;
                    self.best_ignore_bottom_offset = ignore_bottom_offset;
                }

                let kept = (rh - ignore_bottom_offset).max(0);
                let new_height = kept + match_height;
                let mut out = Vec::with_capacity(stride * new_height as usize);
                out.extend_from_slice(&r_raw[..stride * kept as usize]);
                let from = (match_index + 1) as usize * stride;
                out.extend_from_slice(&c_raw[from..from + stride * match_height as usize]);
                let new_result = RgbaImage::from_raw(rw as u32, new_height as u32, out).expect("sized buffer");

                if best_guess {
                    self.status = ScrollingCaptureStatus::PartiallySuccessful;
                } else if self.status != ScrollingCaptureStatus::PartiallySuccessful {
                    self.status = ScrollingCaptureStatus::Successful;
                }
                return Some(new_result);
            }
        }

        self.status = ScrollingCaptureStatus::Failed;
        None
    }
}

/// What the capture loop works on: the selected window and region.
pub trait ScrollTarget {
    /// `selectedWindow.Activate()`.
    fn activate(&mut self) {}
    /// `AutoScrollTop`: Home, and `WM_VSCROLL` `SB_TOP` to the window.
    fn scroll_top(&mut self) {}
    /// `screenshot.CaptureRectangle(selectedRectangle)` without the pointer.
    fn capture(&mut self) -> Option<RgbaImage>;
    /// One scroll step by `method` (`ScrollAmount` notches/keys/messages).
    fn scroll(&mut self, method: ScrollMethod, amount: i32);
    /// `Task.Delay(ms)`.
    fn sleep(&mut self, ms: i32) {
        if ms > 0 {
            std::thread::sleep(Duration::from_millis(ms as u64));
        }
    }
}

/// What a capture ended with.
#[derive(Clone, Debug)]
pub struct CaptureOutcome {
    pub status: ScrollingCaptureStatus,
    pub result: Option<RgbaImage>,
    /// Frames grabbed and scroll steps sent (for tests and logs).
    pub frames: usize,
    pub scrolls: usize,
}

/// `ScrollingCaptureManager.StartCapture` after the region window is shown:
/// activates the window, waits `StartDelay`, scrolls to the top when asked,
/// then grabs, scrolls and stitches until two frames are the same, a frame
/// can't be stitched, or `stop` is set.
pub fn run_capture(options: &ScrollingCaptureOptions, target: &mut dyn ScrollTarget, stop: &AtomicBool) -> CaptureOutcome {
    let mut stitcher = Stitcher::new(options.auto_ignore_bottom_edge);
    stitcher.status = ScrollingCaptureStatus::Failed;
    let mut previous: Option<RgbaImage> = None;
    let (mut frames, mut scrolls) = (0, 0);

    target.activate();
    target.sleep(options.start_delay);

    if options.auto_scroll_top {
        target.scroll_top();
        target.sleep(options.scroll_delay);
    }

    while !stop.load(Ordering::SeqCst) {
        let last = target.capture();
        frames += 1;

        // `CompareLastTwoImages`.
        if let (Some(l), Some(p)) = (&last, &previous)
            && compare_images(l, p)
        {
            break;
        }

        target.scroll(options.scroll_method, options.scroll_amount);
        scrolls += 1;

        let timer = Instant::now();

        if let Some(l) = &last
            && !stitcher.add(l)
        {
            break;
        }

        if stop.load(Ordering::SeqCst) {
            break;
        }

        if last.is_some() {
            previous = last;
        }

        let delay = options.scroll_delay - timer.elapsed().as_millis().min(i32::MAX as u128) as i32;
        if delay > 0 {
            target.sleep(delay);
        }
    }

    CaptureOutcome { status: stitcher.status, result: stitcher.result, frames, scrolls }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn frame(w: u32, h: u32, f: impl Fn(u32, u32) -> [u8; 4]) -> RgbaImage {
        RgbaImage::from_fn(w, h, |x, y| image::Rgba(f(x, y)))
    }

    #[test]
    fn options_round_trip_in_sharex_order() {
        let d = ScrollingCaptureOptions::default();
        let mut v = Value::Null;
        d.write_json(&mut v);
        assert_eq!(
            serde_json::to_string(&v).unwrap(),
            r#"{"StartDelay":300,"AutoScrollTop":false,"ScrollDelay":300,"ScrollMethod":"MouseWheel","ScrollAmount":2,"AutoIgnoreBottomEdge":true,"AutoUpload":false,"ShowRegion":true}"#
        );
        let o = ScrollingCaptureOptions::from_json(&json!({"ScrollMethod": 2, "ScrollAmount": 5}));
        assert_eq!((o.scroll_method, o.scroll_amount, o.start_delay), (ScrollMethod::PageDown, 5, 300));
        assert_eq!(ScrollingCaptureOptions::from_json(&json!({"ScrollMethod": "scrollmessage"})).scroll_method, ScrollMethod::ScrollMessage);
    }

    #[test]
    fn first_frame_is_the_result() {
        let mut s = Stitcher::new(true);
        let f = frame(200, 150, |x, y| [x as u8, y as u8, 0, 255]);
        assert!(s.add(&f));
        assert_eq!(s.status, ScrollingCaptureStatus::Successful);
        assert!(compare_images(s.result.as_ref().unwrap(), &f));
    }

    #[test]
    fn a_wider_frame_fails() {
        let mut s = Stitcher::new(true);
        assert!(s.add(&frame(200, 150, |_, _| [0, 0, 0, 255])));
        assert!(!s.add(&frame(201, 150, |_, _| [0, 0, 0, 255])));
        assert_eq!(s.status, ScrollingCaptureStatus::Failed);
        assert_eq!(s.result.as_ref().unwrap().dimensions(), (200, 150));
    }
}
