//! Runs a case of tests/data/cases.json as tools/editor-harness runs it
//! with ShareX's editor: the base image, the annotations added to an
//! `EditorCore` (images loaded from `ImagePath`, smart erasers filled from
//! the image when `configureFill`), then the case's `ops` (image
//! operations, undo, redo, clone), and the export.

#![allow(dead_code)]

use std::path::Path;

use serde_json::Value;
use sharex_image_editor::editor::{EditorCore, Rect, editor_core};
use sharex_image_editor::model::{CursorType, Kind};
use sharex_image_editor::{Annotation, render};
use sharex_skia::Pixmap;

pub fn load(p: &Path) -> Option<Pixmap> {
    let im = image::open(p).ok()?.to_rgba8();
    Some(Pixmap::from_rgba(im.width(), im.height(), im.as_raw()))
}

/// Why a case can't be checked here, or `None`.
pub fn skip_reason(case: &Value, data: &Path) -> Option<String> {
    if let Some(p) = case.get("pending") {
        return Some(format!("pending: {}", p.as_str().unwrap_or("")));
    }
    // Cases that need a system font (Segoe UI, on Windows) skip without
    // it: the editor would fall back to another font, which ShareX didn't use.
    for f in case
        .get("fonts")
        .and_then(|f| f.as_array())
        .into_iter()
        .flatten()
    {
        let f = f.as_str().unwrap_or("");
        if !render::text::is_installed(f, false, false) {
            return Some(format!("font {f} not installed"));
        }
    }
    // A system cursor (Windows' own, LoadCursor): only where it's the
    // bitmap ShareX drew (tests/data/cursors, dumped by the harness).
    if let Some(t) = case.get("systemCursor").and_then(|t| t.as_str()) {
        let ty: CursorType = serde_json::from_value(Value::String(t.into())).unwrap();
        let expected = std::fs::read(data.join("cursors").join(format!("{t}.bgra"))).ok();
        let live = render::cursor::cursor_bitmap(&ty);
        let same = match (expected, live) {
            (Some(e), Some(l)) => {
                let w = u32::from_le_bytes(e[0..4].try_into().unwrap());
                let h = u32::from_le_bytes(e[4..8].try_into().unwrap());
                let rgba: Vec<u8> = e[8..]
                    .chunks_exact(4)
                    .flat_map(|p| [p[2], p[1], p[0], p[3]])
                    .collect();
                *l == Pixmap::from_premul_rgba(w, h, &rgba)
            }
            _ => false,
        };
        if !same {
            return Some(format!("the system's {t} cursor isn't the one ShareX drew"));
        }
    }
    None
}

fn rect(op: &Value) -> Rect {
    let r = |i: usize| op["rect"][i].as_f64().unwrap() as f32;
    Rect::new(r(0), r(1), r(2), r(3))
}

fn color(op: &Value) -> [u8; 4] {
    let c = render::parse_color(op["color"].as_str().unwrap());
    [c.r, c.g, c.b, c.a]
}

fn num(op: &Value, name: &str) -> f64 {
    op[name].as_f64().unwrap()
}

/// The case's export, as `EditorView.GetSnapshot` after the harness's steps.
pub fn render_case(case: &Value, data: &Path, default_base: &Pixmap) -> Pixmap {
    let base = match case.get("base").and_then(|b| b.as_str()) {
        Some(b) => load(&data.join(b)).unwrap(),
        None => default_base.clone(),
    };
    let configure_fill = case
        .get("configureFill")
        .and_then(|v| v.as_bool())
        .unwrap_or(false);
    let mut core = EditorCore::new(base);
    for json in case["annotations"].as_array().unwrap() {
        let mut a = Annotation::from_json(json).unwrap();
        if let Kind::Image(p) = &mut a.kind
            && !p.image_path.is_empty()
        {
            p.set_image(load(&data.join(&p.image_path)).unwrap());
        }
        if configure_fill && matches!(a.kind, Kind::SmartEraser(_)) {
            let (w, h) = core.canvas_size();
            editor_core::clamp_smart_eraser(&mut a, w as f64, h as f64);
            editor_core::configure_smart_eraser_fill(&mut a, core.source());
        }
        core.add_annotation(a);
    }
    for op in case
        .get("ops")
        .and_then(|o| o.as_array())
        .into_iter()
        .flatten()
    {
        match op["op"].as_str().unwrap() {
            "crop" => {
                core.crop(rect(op));
            }
            "performCrop" => core.perform_crop(),
            "cutOut" => {
                core.cut_out(
                    num(op, "start") as i32,
                    num(op, "end") as i32,
                    op["vertical"].as_bool().unwrap(),
                );
            }
            "performCutOut" => core.perform_cut_out(rect(op), op["vertical"].as_bool().unwrap()),
            "rotate90cw" => {
                core.rotate_90_clockwise();
            }
            "rotate90ccw" => {
                core.rotate_90_counter_clockwise();
            }
            "rotate180" => {
                core.rotate_180();
            }
            "rotate" => {
                let auto = op
                    .get("autoResize")
                    .and_then(|v| v.as_bool())
                    .unwrap_or(true);
                core.rotate_custom_angle(num(op, "angle") as f32, auto);
            }
            "flipH" => {
                core.flip_horizontal();
            }
            "flipV" => {
                core.flip_vertical();
            }
            "resize" => {
                core.resize_image(num(op, "width") as i32, num(op, "height") as i32);
            }
            "resizeCanvas" => {
                let c = color(op);
                core.resize_canvas(
                    num(op, "top") as i32,
                    num(op, "right") as i32,
                    num(op, "bottom") as i32,
                    num(op, "left") as i32,
                    c,
                );
            }
            "autoCrop" => {
                core.auto_crop(num(op, "tolerance") as i32);
            }
            "flatten" => {
                let composite = core.composite();
                core.flatten_image(composite);
            }
            "autoExpand" => {
                let c = color(op);
                core.auto_expand_canvas_to_fit(None, num(op, "margin") as f32, c);
            }
            "undo" => core.undo(),
            "redo" => core.redo(),
            "clone" => {
                let r = rect(op);
                core.clone_region((
                    r.l as f64,
                    r.t as f64,
                    (r.r - r.l) as f64,
                    (r.b - r.t) as f64,
                ));
            }
            other => panic!("unknown op {other}"),
        }
    }
    core.composite()
}
