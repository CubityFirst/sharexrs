use super::*;

fn ws() -> Workspace {
    let mut pm = Pixmap::new(400, 300);
    pm.pixels_mut().iter_mut().for_each(|p| *p = 0xFF80_8080);
    let mut w = Workspace::new(pm, ImageEditorOptions::default());
    w.select_tool(EditorTool::Rectangle);
    w
}

fn drag(w: &mut Workspace, from: P, to: P, m: Mods) {
    w.pointer_moved(from, m);
    w.pointer_pressed(from, Button::Left, m);
    for i in 1..=4 {
        let t = i as f64 / 4.0;
        w.pointer_moved((from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t), m);
    }
    w.pointer_released(to, Button::Left, m);
}

#[test]
fn a_drag_draws_a_selected_rectangle_from_the_options() {
    let mut w = ws();
    drag(&mut w, (50.0, 40.0), (150.0, 120.0), Mods::NONE);
    assert_eq!(w.annotations().len(), 1);
    let a = &w.annotations()[0];
    assert_eq!(a.common.start_point, Point::new(50.0, 40.0));
    assert_eq!(a.common.end_point, Point::new(150.0, 120.0));
    assert_eq!(a.common.stroke_color, "#FFF23C3C");
    assert_eq!(a.common.stroke_width, 4.0);
    assert_eq!(w.selected().map(|a| a.common.id.clone()), Some(a.common.id.clone()));
    let (handles, line) = w.handles();
    assert_eq!(handles.len(), 9);
    assert_eq!(handles[0].at, (50.0, 40.0));
    assert_eq!(handles[8].kind, HandleKind::Rotate);
    assert_eq!(handles[8].at, (100.0, 10.0));
    assert_eq!(line, Some(((100.0, 40.0), (100.0, 10.0))));
    assert!(w.is_dirty());
}

#[test]
fn tiny_shapes_are_dropped_and_shift_makes_squares() {
    let mut w = ws();
    drag(&mut w, (50.0, 40.0), (53.0, 43.0), Mods::NONE);
    assert!(w.annotations().is_empty());
    let shift = Mods { shift: true, ..Mods::NONE };
    drag(&mut w, (50.0, 40.0), (150.0, 80.0), shift);
    assert_eq!(w.annotations()[0].common.end_point, Point::new(150.0, 140.0));
}

#[test]
fn a_shape_of_the_tool_moves_and_undo_and_redo_restore() {
    let mut w = ws();
    drag(&mut w, (50.0, 40.0), (150.0, 120.0), Mods::NONE);
    w.clear_selection();
    // Pressing on it with its tool drags it.
    drag(&mut w, (100.0, 80.0), (110.0, 90.0), Mods::NONE);
    assert_eq!(w.annotations().len(), 1);
    assert_eq!(w.annotations()[0].common.start_point, Point::new(60.0, 50.0));
    w.undo();
    assert!(w.annotations().is_empty());
    w.redo();
    assert_eq!(w.annotations().len(), 1);
    // Ctrl draws over it instead.
    let ctrl = Mods { ctrl: true, ..Mods::NONE };
    drag(&mut w, (100.0, 80.0), (200.0, 200.0), ctrl);
    assert_eq!(w.annotations().len(), 2);
}

#[test]
fn delete_removes_the_selection_and_steps_renumber() {
    let mut w = ws();
    w.select_tool(EditorTool::Step);
    for x in [50.0, 150.0, 250.0] {
        w.pointer_pressed((x, 100.0), Button::Left, Mods::NONE);
        w.pointer_released((x, 100.0), Button::Left, Mods::NONE);
    }
    let numbers = |w: &Workspace| -> Vec<i32> {
        w.annotations().iter().map(|a| if let Kind::Number(p) = &a.kind { p.number } else { 0 }).collect()
    };
    assert_eq!(numbers(&w), vec![1, 2, 3]);
    assert!(w.delete_at((50.0, 100.0)));
    assert_eq!(numbers(&w), vec![1, 2]);
    assert_eq!(w.number_counter(), 3);
    let none = |_: Key, _: Mods| None;
    w.key_down(Key::Delete, Mods { shift: true, ..Mods::NONE }, &none);
    assert!(w.annotations().is_empty());
    w.key_down(Key::Letter('Z'), Mods { ctrl: true, ..Mods::NONE }, &none);
    assert_eq!(numbers(&w), vec![1, 2]);
}

#[test]
fn text_is_typed_in_place_and_committed() {
    let mut w = ws();
    w.select_tool(EditorTool::Text);
    w.pointer_pressed((40.0, 50.0), Button::Left, Mods::NONE);
    w.pointer_released((40.0, 50.0), Button::Left, Mods::NONE);
    assert!(w.text_edit().is_some());
    w.text_input("Hi");
    let none = |_: Key, _: Mods| None;
    w.key_down(Key::Enter, Mods::NONE, &none);
    assert!(w.text_edit().is_none());
    let a = &w.annotations()[0];
    let Kind::Text(t) = &a.kind else { panic!() };
    assert_eq!(t.text, "Hi");
    assert_eq!(a.common.start_point, Point::new(40.0, 50.0));
    assert!(a.common.end_point.x > 40.0 + 16.0);
    assert!(w.selected().is_some());
}

#[test]
fn hotkeys_pick_tools_and_load_their_options() {
    let mut w = ws();
    let keys = |k: Key, m: Mods| (k == Key::Letter('T') && m == Mods::NONE).then_some(EditorTool::Text);
    let r = w.key_down(Key::Letter('T'), Mods::NONE, &keys);
    assert_eq!(r.tool, Some(EditorTool::Text));
    assert_eq!(w.active_tool(), EditorTool::Text);
    assert_eq!(w.tools.stroke_width, 8);
    assert_eq!(w.tools.options.LastUsedAnnotationTool, EditorTool::Text);
}

#[test]
fn lines_snap_to_45_degrees_and_get_end_handles() {
    let mut w = ws();
    w.select_tool(EditorTool::Line);
    drag(&mut w, (100.0, 100.0), (200.0, 190.0), Mods { shift: true, ..Mods::NONE });
    let a = &w.annotations()[0];
    let e = a.common.end_point;
    assert!((e.x - e.y).abs() < 0.01);
    let (handles, _) = w.handles();
    assert_eq!(handles.iter().map(|h| h.kind).collect::<Vec<_>>(), vec![
        HandleKind::SegmentStart,
        HandleKind::SegmentEnd,
        HandleKind::SegmentCenter
    ]);
}

#[test]
fn the_composite_renders_and_the_snapshot_crops() {
    let mut w = ws();
    drag(&mut w, (10.0, 10.0), (60.0, 60.0), Mods::NONE);
    let region = render::render_region(w.source(), w.annotations(), (5.0, 5.0, 100.0, 80.0)).unwrap();
    let snap = w.snapshot(5, 5, 100, 80);
    assert_eq!(snap, region);
    let full = render::render(w.source(), w.annotations());
    assert_eq!(&w.export(), &full);
}

fn none(_: Key, _: Mods) -> Option<EditorTool> {
    None
}

const CTRL: Mods = Mods { shift: false, ctrl: true, alt: false };
const CTRL_SHIFT: Mods = Mods { shift: true, ctrl: true, alt: false };

#[test]
fn copy_paste_and_duplicate_offset_by_20_and_select_the_copy() {
    let mut w = ws();
    drag(&mut w, (50.0, 40.0), (150.0, 120.0), Mods::NONE);
    assert!(w.key_down(Key::Letter('C'), CTRL_SHIFT, &none).handled);
    // Ctrl+V is the host's (the system clipboard first).
    assert_eq!(w.key_down(Key::Letter('V'), CTRL, &none).command, Some(Command::Paste));
    assert!(w.paste_annotation());
    assert_eq!(w.annotations().len(), 2);
    assert_eq!(w.annotations()[1].common.start_point, Point::new(70.0, 60.0));
    assert_eq!(w.selected().unwrap().common.key.0, w.annotations()[1].common.key.0);
    // The copy keeps ShareX's `Id` (`Annotation.Clone`), as its own object.
    assert_eq!(w.annotations()[0].common.id, w.annotations()[1].common.id);
    assert_ne!(w.annotations()[0].common.key.0, w.annotations()[1].common.key.0);
    w.key_down(Key::Letter('D'), CTRL, &none);
    assert_eq!(w.annotations()[2].common.start_point, Point::new(90.0, 80.0));
    // Cut deletes, undo brings it back.
    w.key_down(Key::Letter('X'), CTRL, &none);
    assert_eq!(w.annotations().len(), 2);
    w.undo();
    assert_eq!(w.annotations().len(), 3);
    // Undo selects again by `SelectedAnnotationId`: the first annotation
    // with the cut one's `Id`, the original.
    assert_eq!(w.selected().unwrap().common.key.0, w.annotations()[0].common.key.0);
}

#[test]
fn z_order_keys_move_the_selection_through_the_list() {
    let mut w = ws();
    drag(&mut w, (10.0, 10.0), (60.0, 60.0), Mods::NONE);
    drag(&mut w, (100.0, 10.0), (160.0, 60.0), Mods::NONE);
    let first = w.annotations()[0].common.id.clone();
    w.clear_selection();
    w.pointer_pressed((30.0, 10.0), Button::Left, Mods::NONE);
    w.pointer_released((30.0, 10.0), Button::Left, Mods::NONE);
    assert_eq!(w.selected().unwrap().common.id, first);
    w.key_down(Key::Home, Mods::NONE, &none);
    assert_eq!(w.annotations()[1].common.id, first);
    w.key_down(Key::PageDown, Mods::NONE, &none);
    assert_eq!(w.annotations()[0].common.id, first);
}

#[test]
fn a_quick_crop_cuts_the_image_and_moves_the_annotations_and_undo_restores_it() {
    let mut w = ws();
    drag(&mut w, (150.0, 150.0), (200.0, 200.0), Mods::NONE);
    drag(&mut w, (10.0, 10.0), (30.0, 30.0), Mods::NONE);
    w.select_tool(EditorTool::Crop);
    w.pointer_moved((100.0, 100.0), Mods::NONE);
    w.pointer_pressed((100.0, 100.0), Button::Left, Mods::NONE);
    w.pointer_moved((250.0, 220.0), Mods::NONE);
    let c = w.crop_chrome().unwrap();
    assert_eq!(c.rect, (100.0, 100.0, 150.0, 120.0));
    assert_eq!(c.info.as_deref(), Some("X: 100 Y: 100 W: 150 H: 120"));
    assert_eq!(c.shades.len(), 4);
    assert_eq!(c.guides.len(), 4);
    w.pointer_released((250.0, 220.0), Button::Left, Mods::NONE);
    assert_eq!(w.size(), (150, 120));
    // The one outside is gone, the other moved.
    assert_eq!(w.annotations().len(), 1);
    assert_eq!(w.annotations()[0].common.start_point, Point::new(50.0, 50.0));
    w.undo();
    assert_eq!(w.size(), (400, 300));
    assert_eq!(w.annotations().len(), 2);
    w.redo();
    assert_eq!(w.size(), (150, 120));
}

#[test]
fn without_quick_crop_the_crop_waits_and_enter_confirms() {
    let o = ImageEditorOptions { QuickCrop: false, ..Default::default() };
    let mut w = Workspace::new(Pixmap::new(400, 300), o);
    w.select_tool(EditorTool::Crop);
    drag(&mut w, (100.0, 100.0), (200.0, 200.0), Mods::NONE);
    let c = w.crop_chrome().unwrap();
    assert!(c.pending);
    assert_eq!(c.handles.len(), 8);
    // The bottom-right handle drags it bigger.
    drag(&mut w, (200.0, 200.0), (220.0, 250.0), Mods::NONE);
    assert_eq!(w.crop_chrome().unwrap().rect, (100.0, 100.0, 120.0, 150.0));
    w.key_down(Key::Enter, Mods::NONE, &none);
    assert_eq!(w.size(), (120, 150));
    assert!(w.crop_chrome().is_none());
}

#[test]
fn cut_out_removes_a_strip_across_the_drag() {
    let mut pm = Pixmap::new(400, 300);
    for x in 0..400 {
        *pm.pixel_mut(x, 0) = 0xFF00_0000 | x as u32;
    }
    let mut w = Workspace::new(pm, ImageEditorOptions::default());
    w.select_tool(EditorTool::CutOut);
    drag(&mut w, (100.0, 50.0), (160.0, 55.0), Mods::NONE);
    assert_eq!(w.size(), (340, 300));
    assert_eq!(w.source().pixel(99, 0) & 0xFFFF, 99);
    assert_eq!(w.source().pixel(100, 0) & 0xFFFF, 160);
}

#[test]
fn clone_copies_the_region_into_a_selected_image_annotation() {
    let mut w = ws();
    w.select_tool(EditorTool::Clone);
    drag(&mut w, (10.0, 20.0), (60.0, 50.0), Mods::NONE);
    let a = w.selected().unwrap().clone();
    assert!(matches!(a.kind, Kind::Image(_)));
    assert_eq!(a.common.tool_type, EditorTool::Clone);
    assert_eq!(a.common.start_point, Point::new(10.0, 20.0));
    assert_eq!(w.image(&a.common.key.0).map(|p| (p.width(), p.height())), Some((50, 30)));
    assert_eq!(w.active_tool(), EditorTool::Clone);
}

#[test]
fn flatten_draws_the_annotations_into_the_image() {
    let mut w = ws();
    drag(&mut w, (10.0, 10.0), (60.0, 60.0), Mods::NONE);
    let composite = w.composite().clone();
    w.key_down(Key::Letter('F'), CTRL_SHIFT, &none);
    assert!(w.annotations().is_empty());
    assert_eq!(w.source(), &composite);
    w.undo();
    assert_eq!(w.annotations().len(), 1);
}

#[test]
fn the_right_button_deletes_with_the_option_or_selects_for_the_menu() {
    let mut w = ws();
    drag(&mut w, (10.0, 10.0), (60.0, 60.0), Mods::NONE);
    w.clear_selection();
    assert_eq!(w.right_pressed((10.0, 30.0)), RightClick::ContextMenu);
    assert!(w.selected().is_some());
    w.tools.options.RightClickDeletesShape = true;
    assert_eq!(w.right_pressed((10.0, 30.0)), RightClick::Deleted);
    assert!(w.annotations().is_empty());
    assert_eq!(w.right_pressed((10.0, 30.0)), RightClick::ContextMenu);
}

#[test]
fn a_drawn_balloon_opens_its_editor_with_a_default_tail_and_typed_text_is_kept() {
    let mut w = ws();
    w.select_tool(EditorTool::SpeechBalloon);
    drag(&mut w, (200.0, 100.0), (210.0, 110.0), Mods::NONE);
    // Too small: grown to 200x100, the tail below its middle.
    let a = w.selected().unwrap().clone();
    assert_eq!(a.common.end_point, Point::new(400.0, 200.0));
    let Kind::SpeechBalloon(p) = &a.kind else { panic!() };
    assert_eq!(p.tail_point, Point::new(300.0, 230.0));
    assert_eq!(w.text_edit().map(|t| t.kind), Some(TextKind::Balloon));
    let (handles, _) = w.handles();
    assert_eq!(handles.last().map(|h| (h.kind, h.at)), Some((HandleKind::BalloonTail, (300.0, 230.0))));
    w.text_input("Hi");
    w.key_down(Key::Enter, Mods::NONE, &none);
    let Kind::SpeechBalloon(p) = &w.annotations()[0].kind else { panic!() };
    assert_eq!(p.text, "Hi");
}

#[test]
fn a_double_click_edits_text_in_its_own_box() {
    let mut w = ws();
    w.select_tool(EditorTool::Text);
    w.pointer_pressed((40.0, 50.0), Button::Left, Mods::NONE);
    w.pointer_released((40.0, 50.0), Button::Left, Mods::NONE);
    w.text_input("Hi");
    w.key_down(Key::Enter, Mods::NONE, &none);
    let end = w.annotations()[0].common.end_point;
    for _ in 0..2 {
        w.pointer_pressed((50.0, 60.0), Button::Left, Mods::NONE);
        w.pointer_released((50.0, 60.0), Button::Left, Mods::NONE);
    }
    assert_eq!(w.text_edit().map(|t| t.kind), Some(TextKind::Edit));
    w.text_input("!");
    w.key_down(Key::Enter, Mods::NONE, &none);
    let a = &w.annotations()[0];
    let Kind::Text(t) = &a.kind else { panic!() };
    assert_eq!(t.text, "Hi!");
    // Its rectangle stays.
    assert_eq!(a.common.end_point, end);
}

#[test]
fn the_image_and_emoji_tools_ask_the_host() {
    let mut w = ws();
    w.select_tool(EditorTool::Image);
    w.select_tool(EditorTool::Emoji);
    assert_eq!(w.take_requests(), vec![Request::InsertImage, Request::EmojiPicker]);
    assert_eq!(w.active_tool(), EditorTool::Rectangle);
    let id = w.insert_emoji("1f600", "grinning face", (10.0, 10.0));
    assert_eq!(w.active_tool(), EditorTool::Select);
    assert_eq!(w.selected().unwrap().common.key.0, id);
    assert_eq!(w.selected().unwrap().common.end_point, Point::new(170.0, 170.0));
}

#[test]
fn the_smart_eraser_stretches_matching_edges() {
    // Columns 0..200 one colour, the rest another: a box across the
    // boundary has differing left and right columns but equal rows.
    let mut pm = Pixmap::new(400, 300);
    for y in 0..300 {
        for x in 0..400 {
            *pm.pixel_mut(x, y) = if x < 200 { 0xFFFF_0000 } else { 0xFF00_00FF };
        }
    }
    let mut w = Workspace::new(pm, ImageEditorOptions::default());
    w.select_tool(EditorTool::SmartEraser);
    drag(&mut w, (150.0, 100.0), (250.0, 150.0), Mods::NONE);
    let a = &w.annotations()[0];
    assert_eq!(a.common.fill_color, "#FF0000");
    let Kind::SmartEraser(p) = &a.kind else { panic!() };
    assert_eq!(p.fill_mode, crate::model::SmartEraserFillMode::StretchVertically);
    assert_eq!(p.edge_pixels.as_ref().map(|e| e.len()), Some(100));
}

#[test]
fn a_long_new_text_grows_the_canvas_while_typed_then_fits_it_once() {
    // (Needs a text font: Segoe UI or the bundled fallback.)
    let mut w = ws();
    w.select_tool(EditorTool::Text);
    w.pointer_pressed((300.0, 250.0), Button::Left, Mods::NONE);
    w.pointer_released((300.0, 250.0), Button::Left, Mods::NONE);
    if w.text_edit().and_then(|t| text_box(t)).is_none() {
        return;
    }
    w.text_input("A rather long caption that runs off the image");
    let (lw, lh) = w.size();
    assert!(lw > 400 && lh > 300, "grown live to {lw}x{lh}");
    let none = |_: Key, _: Mods| None;
    w.key_down(Key::Enter, Mods::NONE, &none);
    // The live growth is replaced by the exact fit (16px to spare), one
    // undo step.
    let (fw, fh) = w.size();
    assert!(fw > 400 && fw < lw, "fit {fw} (live {lw})");
    assert!(fh > 300);
    w.undo();
    assert_eq!(w.size(), (400, 300));
}
