//! `sharexrs --render controls-gallery [--theme dark|light] --out <png>`:
//! every shared control in every state, offscreen, laid out as the
//! Avalonia.Headless probe that rendered ShareX's controls for the review
//! (`shots/review-controls2/Program.cs`): each a 420x90 panel-coloured
//! canvas with the control at 20,20 (toggles 52x32, check boxes and the
//! slider 32 tall, the slider 362 wide, everything else 130x40). The
//! sheet has a row per control ([`KINDS`]) and a column per state
//! ([`STATES`]), each cell one canvas.
//!
//! States are driven with real input: the pointer over the control
//! (hover), held down on it (pressed), Tab (keyboard focus) and
//! `enabled: false`.

use egui::{Color32, Event, Id, Key, PointerButton, Rect, pos2, vec2};
use sharex_ui::controls::{Button, CheckBox, ColorButton, ComboBox, ComboItem, NumericUpDown, Slider, TextBox, ToggleSwitch};
use sharex_ui::offscreen::Offscreen;
use sharex_ui::theme::{Thickness, palette};

pub const KINDS: [&str; 10] = ["toggle-off", "toggle-on", "check-off", "check-on", "button", "combo", "text", "number", "slider", "color"];
pub const STATES: [&str; 5] = ["rest", "hover", "pressed", "focus", "disabled"];
pub const CELL: [u32; 2] = [420, 90];

/// The control's rect on the canvas.
pub fn control_rect(kind: &str) -> Rect {
    let size = match kind {
        k if k.starts_with("toggle") => vec2(52.0, 32.0),
        k if k.starts_with("check") => vec2(130.0, 32.0),
        "slider" => vec2(362.0, 32.0),
        _ => vec2(130.0, 40.0),
    };
    Rect::from_min_size(pos2(20.0, 20.0), size)
}

/// Where the pointer goes for hover and press: as the probe sets
/// `:pointerover`, on the control (the thumb of the slider, the increase
/// button of the numeric box).
fn pointer(kind: &str) -> egui::Pos2 {
    match kind {
        "slider" => pos2(201.0, 34.0),
        "number" => pos2(98.0, 40.0),
        _ => pos2(30.0, 36.0),
    }
}

/// Draws one control in one state.
pub fn render_cell(dark: bool, kind: &str, state: &str) -> Result<image::RgbaImage, String> {
    let mut screen = Offscreen::new(CELL, 1.0, dark)?;
    let enabled = state != "disabled";
    let id = Id::new("gallery-control");
    let rect = control_rect(kind);
    let mut text = String::from("Example");
    let mut ui = |ui: &mut egui::Ui| {
        let p = palette(ui.ctx());
        ui.painter().rect_filled(Rect::from_min_size(pos2(0.0, 0.0), vec2(CELL[0] as f32, CELL[1] as f32)), 0.0, p.background_panel);
        // Values are reset each frame, so holding a control down shows
        // its pressed state rather than what the press does.
        let mut on = kind.ends_with("-on");
        match kind {
            "toggle-off" | "toggle-on" => {
                ToggleSwitch { enabled }.show(ui, rect.min, id, &mut on);
            }
            "check-off" | "check-on" => {
                let mut c = CheckBox::new("Example");
                c.enabled = enabled;
                c.show(ui, rect, id, &mut on);
            }
            "button" => {
                Button::default().show(ui, rect, id, "Example", enabled);
            }
            "combo" => {
                ComboBox { padding: Thickness::hv(8.0, 6.0), enabled }.show(ui, rect, id, &[ComboItem::text("Dark"), ComboItem::text("Light")], Some(0));
                ComboBox::set_open(ui.ctx(), id, false);
            }
            "text" => {
                TextBox { padding: Thickness::hv(8.0, 6.0), enabled, ..Default::default() }.show(ui, rect, id, &mut text);
            }
            "number" => {
                let mut v = 10;
                NumericUpDown { minimum: 0, maximum: 100, enabled }.show(ui, rect, id, &mut v);
            }
            "slider" => {
                let mut v = 50.0;
                Slider { minimum: 0.0, maximum: 100.0, step: 0.0, enabled }.show(ui, rect, id, &mut v);
            }
            _ => {
                ColorButton { enabled }.show(ui, rect, id, Color32::from_rgb(0x09, 0xAA, 0xFF));
            }
        }
    };
    for _ in 0..3 {
        screen.run(vec![Event::PointerGone], &mut ui);
    }
    let at = pointer(kind);
    match state {
        "hover" | "pressed" => {
            for _ in 0..3 {
                screen.run(vec![Event::PointerMoved(at)], &mut ui);
            }
            if state == "pressed" {
                let press = Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() };
                screen.run(vec![press], &mut ui);
                screen.run(Vec::new(), &mut ui);
            }
        }
        "focus" => {
            let tab = Event::Key { key: Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
            screen.run(vec![tab], &mut ui);
            for _ in 0..2 {
                screen.run(Vec::new(), &mut ui);
            }
        }
        _ => {}
    }
    Ok(screen.draw())
}

/// The whole sheet for one theme.
pub fn render(dark: bool) -> Result<image::RgbaImage, String> {
    let [w, h] = CELL;
    let mut sheet = image::RgbaImage::new(w * STATES.len() as u32, h * KINDS.len() as u32);
    for (row, kind) in KINDS.iter().enumerate() {
        for (col, state) in STATES.iter().enumerate() {
            let cell = render_cell(dark, kind, state)?;
            image::imageops::replace(&mut sheet, &cell, (col as u32 * w) as i64, (row as u32 * h) as i64);
        }
    }
    Ok(sheet)
}

#[cfg(test)]
mod tests {
    //! Pixels and geometry measured on ShareX (the review's Avalonia
    //! probe, `av-*.png`, and screenshots of the real app). The GPU dithers
    //! by a level or two, so colours compare within 3.
    use super::*;

    fn px(img: &image::RgbaImage, x: u32, y: u32) -> [u8; 3] {
        let p = img.get_pixel(x, y).0;
        [p[0], p[1], p[2]]
    }

    fn near(a: [u8; 3], rgb: u32) -> bool {
        a.iter().zip([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]).all(|(x, y)| x.abs_diff(y) <= 3)
    }

    #[track_caller]
    fn assert_px(img: &image::RgbaImage, x: u32, y: u32, rgb: u32) {
        let a = px(img, x, y);
        assert!(near(a, rgb), "({x},{y}) is {a:02X?}, expected {rgb:06X}");
    }

    #[test]
    fn toggle_on_keeps_the_gradient_under_the_pointer() {
        // F6: #09A7FA at (25,35) at rest, hovered and pressed, both themes.
        for dark in [true, false] {
            for state in ["rest", "hover", "pressed"] {
                assert_px(&render_cell(dark, "toggle-on", state).unwrap(), 25, 35, 0x09A7FA);
            }
        }
    }

    #[test]
    fn slider_thumb_sits_a_pixel_above_the_track() {
        // F9: track rows 33-34, thumb centred at (201,33); the 4px ring
        // narrows to 3 only with the pointer over the thumb.
        let rest = render_cell(true, "slider", "rest").unwrap();
        assert_px(&rest, 100, 33, 0x09AAFF);
        assert_px(&rest, 100, 34, 0x09AAFF);
        assert_px(&rest, 100, 32, 0x242424);
        assert_px(&rest, 100, 35, 0x242424);
        assert_px(&rest, 201, 33, 0x09AAFF);
        assert_px(&rest, 201, 26, 0x2E2E2E);
        let hover = render_cell(true, "slider", "hover").unwrap();
        assert_px(&hover, 201, 27, 0x09AAFF);
        // Disabled: #333333 tracks, the accent thumb.
        let disabled = render_cell(true, "slider", "disabled").unwrap();
        assert_px(&disabled, 100, 33, 0x333333);
        assert_px(&disabled, 300, 33, 0x333333);
        assert_px(&disabled, 201, 33, 0x09AAFF);
    }

    #[test]
    fn check_mark_support_matches_sharex() {
        // F8: the mark's strong pixels (as dark as #404040 on average) span
        // 10x8, 5,6 into the box, as on ShareX's General page.
        for dark in [true, false] {
            let img = render_cell(dark, "check-on", "rest").unwrap();
            let strong: Vec<(u32, u32)> = (28..44).flat_map(|y| (22..38).map(move |x| (x, y))).filter(|&(x, y)| px(&img, x, y).iter().map(|&c| c as u32).sum::<u32>() < 3 * 0x40).collect();
            let (xs, ys): (Vec<u32>, Vec<u32>) = strong.into_iter().unzip();
            assert_eq!((*xs.iter().min().unwrap(), *ys.iter().min().unwrap()), (25, 32));
            assert_eq!((*xs.iter().max().unwrap(), *ys.iter().max().unwrap()), (34, 39));
        }
    }

    #[test]
    fn combo_chevron_rows() {
        // F4: the glyph's cell is (128,34,12,12); ShareX inks rows 36-42.
        let img = render_cell(true, "combo", "rest").unwrap();
        let rows: Vec<u32> = (30..50).filter(|&y| (128..140).any(|x| !near(px(&img, x, y), 0x2E2E2E))).collect();
        assert_eq!((rows[0], *rows.last().unwrap()), (36, 42));
    }

    #[test]
    fn keyboard_focus_states() {
        // F12: the button's adorner, 2px white then 1px #99000000 inside.
        let button = render_cell(true, "button", "focus").unwrap();
        assert_px(&button, 20, 20, 0xFFFFFF);
        assert_px(&button, 21, 40, 0xFFFFFF);
        assert_px(&button, 22, 40, 0x121212);
        assert_px(&button, 23, 40, 0x2E2E2E);
        // F11: the combo box's accent highlight (60% in Dark), no adorner.
        let combo = render_cell(true, "combo", "focus").unwrap();
        assert_px(&combo, 60, 40, 0x1778AB);
        assert_px(&combo, 20, 40, 0x174862);
        // F11: a focused text box's border is 2px.
        let text = render_cell(true, "text", "focus").unwrap();
        assert_px(&text, 21, 40, 0x1F1F1F);
        assert_px(&render_cell(true, "text", "rest").unwrap(), 21, 40, 0x2E2E2E);
    }

    #[test]
    fn button_text_and_disabled_layer() {
        // F13: hovered text is white; disabled, the presenter (no border)
        // at 35%: #2E2E2E over #242424 becomes #272727.
        let hover = render_cell(true, "button", "hover").unwrap();
        let brightest = (28..52).flat_map(|y| (40..110).map(move |x| (x, y))).map(|(x, y)| px(&hover, x, y)[1]).max().unwrap();
        assert!(brightest >= 0xF8, "{brightest:02X}");
        let disabled = render_cell(true, "button", "disabled").unwrap();
        assert_px(&disabled, 40, 25, 0x272727);
        assert_px(&disabled, 40, 20, 0x252525);
    }

    #[test]
    fn tab_moves_visible_focus_and_a_click_hides_it() {
        let mut screen = Offscreen::new(CELL, 1.0, true).unwrap();
        let mut ui = |ui: &mut egui::Ui| {
            let p = palette(ui.ctx());
            ui.painter().rect_filled(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 90.0)), 0.0, p.background_panel);
            Button::default().show(ui, Rect::from_min_size(pos2(20.0, 20.0), vec2(130.0, 40.0)), Id::new("a"), "A", true);
            Button::default().show(ui, Rect::from_min_size(pos2(200.0, 20.0), vec2(130.0, 40.0)), Id::new("b"), "B", true);
        };
        let tab = || Event::Key { key: Key::Tab, physical_key: None, pressed: true, repeat: false, modifiers: Default::default() };
        screen.run(Vec::new(), &mut ui);
        screen.run(vec![tab()], &mut ui);
        screen.run(Vec::new(), &mut ui);
        let first = screen.draw();
        assert_px(&first, 20, 20, 0xFFFFFF);
        assert_px(&first, 200, 20, 0x242424);
        screen.run(vec![tab()], &mut ui);
        screen.run(Vec::new(), &mut ui);
        let second = screen.draw();
        assert_px(&second, 20, 20, 0x242424);
        assert_px(&second, 200, 20, 0xFFFFFF);
        let at = pos2(5.0, 5.0);
        screen.run(vec![Event::PointerMoved(at), Event::PointerButton { pos: at, button: PointerButton::Primary, pressed: true, modifiers: Default::default() }], &mut ui);
        screen.run(Vec::new(), &mut ui);
        assert_px(&screen.draw(), 200, 20, 0x242424);
    }

    #[test]
    fn numeric_endpoint_disables_its_button() {
        // F10: at the maximum the increase button is disabled: base 20%
        // over the control background, its glyph base 40%.
        let mut screen = Offscreen::new(CELL, 1.0, true).unwrap();
        let img = screen.render(|ui| {
            let p = palette(ui.ctx());
            ui.painter().rect_filled(Rect::from_min_size(pos2(0.0, 0.0), vec2(420.0, 90.0)), 0.0, p.background_panel);
            let mut v = 100;
            NumericUpDown { minimum: 0, maximum: 100, enabled: true }.show(ui, control_rect("number"), Id::new("n"), &mut v);
        });
        assert_px(&img, 90, 25, 0x585858);
        assert_px(&img, 125, 25, 0x2E2E2E);
    }
}

#[cfg(test)]
mod settings_page_tests {
    //! Sol's review coordinates on Application settings › Theme (950x720,
    //! Dark), against ShareX's screenshot of the same page.
    #[test]
    fn theme_page_controls_match_sharex() {
        let out = std::env::temp_dir().join(format!("sharexrs-theme-{}.png", std::process::id()));
        let args: Vec<String> = ["settings-theme", "--theme", "dark", "--out", out.to_str().unwrap()].iter().map(|s| s.to_string()).collect();
        crate::render::run(&args).unwrap();
        let img = image::open(&out).unwrap().to_rgb8();
        let _ = std::fs::remove_file(&out);
        let px = |x, y| img.get_pixel(x, y).0;
        let near = |x: u32, y: u32, rgb: u32| {
            let a = px(x, y);
            assert!(a.iter().zip([(rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8]).all(|(p, q)| p.abs_diff(q) <= 3), "({x},{y}) is {a:02X?}, ShareX {rgb:06X}");
        };
        // F1: the selected page's corner, one r4 shape with its inset outline.
        near(9, 93, 0x195371);
        near(10, 93, 0x0B9EEA);
        near(8, 94, 0x195371);
        // F4: the theme combo's chevron inks rows 168-174 of its cell.
        let rows: Vec<u32> = (160..186).filter(|&y| (567..579).any(|x| px(x, y).iter().zip(px(560, 172)).any(|(a, b)| a.abs_diff(b) > 8))).collect();
        assert_eq!((rows[0], *rows.last().unwrap()), (168, 174));
        // F9: the spacing slider's track on rows 348-349, its thumb centred
        // on (640,348) with a 4px ring.
        near(500, 348, 0x09AAFF);
        near(500, 349, 0x09AAFF);
        near(640, 348, 0x09AAFF);
        near(640, 341, 0x2E2E2E);
        // F3: a check box's corner (261,123) as ShareX antialiases it.
        near(262, 123, 0x252525);
    }
}
