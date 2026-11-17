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
    let full = render::render(w.source(), w.annotations());
    let snap = w.snapshot(5, 5, 100, 80);
    assert_eq!(snap.pixel(5, 5), full.pixel(10, 10));
    assert_eq!(snap.pixel(99, 79), full.pixel(104, 84));
}
