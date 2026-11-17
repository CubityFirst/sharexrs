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
