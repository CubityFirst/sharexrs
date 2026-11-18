use super::*;

fn session(windows: Vec<SimpleWindowInfo>) -> RegionCapture {
    let mut shot = RgbaImage::new(1024, 768);
    for (x, y, p) in shot.enumerate_pixels_mut() {
        *p = image::Rgba([(x % 256) as u8, (y % 256) as u8, 70, 255]);
    }
    let screen = Screen { bounds: Rect::new(0, 0, 1024, 768), work_area: Rect::new(0, 0, 1024, 720), scale: 1.0, primary: true };
    RegionCapture::new(Request {
        screenshot: shot,
        screen_bounds: screen.bounds,
        screens: vec![screen],
        options: RegionCaptureOptions::default(),
        windows,
        cursor: Some((300, 300)),
        last_regions: Vec::new(),
        enable_annotations: true,
        editor_options: Default::default(),
        cursor_image: None,
    })
}

fn none() -> Modifiers {
    Modifiers::default()
}

fn drag(s: &mut RegionCapture, from: (f64, f64), to: (f64, f64)) -> Outcome {
    s.pointer_moved(PointF::new(from.0, from.1), none());
    s.pointer_pressed(PointF::new(from.0, from.1), Button::Left, none(), 1.0);
    for i in 1..=10 {
        let t = i as f64 / 10.0;
        s.pointer_moved(PointF::new(from.0 + (to.0 - from.0) * t, from.1 + (to.1 - from.1) * t), none());
    }
    s.pointer_released(PointF::new(to.0, to.1), Button::Left, none())
}

#[test]
fn dragging_captures_both_end_pixels() {
    let mut s = session(Vec::new());
    let Outcome::Captured(r) = drag(&mut s, (200.0, 150.0), (600.0, 450.0)) else { panic!("not captured") };
    assert_eq!(r.screen_rectangle, Rect::new(200, 150, 401, 301));
    assert_eq!(r.image.dimensions(), (401, 301));
    assert_eq!(r.image.get_pixel(0, 0), &image::Rgba([200, 150, 70, 255]));
    assert_eq!(r.window_handle, 0);
}

#[test]
fn clicking_a_window_captures_it_whole() {
    let w = SimpleWindowInfo { handle: 42, rect: Rect::new(300, 200, 400, 300), is_window: true };
    let mut s = session(vec![w]);
    s.pointer_moved(PointF::new(650.0, 420.0), none());
    assert_eq!(s.hover, RectF::new(300.0, 200.0, 400.0, 300.0));
    s.pointer_pressed(PointF::new(650.0, 420.0), Button::Left, none(), 1.0);
    // Less than 4 px of movement keeps the window.
    s.pointer_moved(PointF::new(652.0, 421.0), none());
    let Outcome::Captured(r) = s.pointer_released(PointF::new(652.0, 421.0), Button::Left, none()) else {
        panic!("not captured")
    };
    assert_eq!(r.screen_rectangle, Rect::new(300, 200, 400, 300));
    assert_eq!(r.window_handle, 42);
}

#[test]
fn dragging_from_a_window_draws_a_new_region() {
    let w = SimpleWindowInfo { handle: 42, rect: Rect::new(300, 200, 400, 300), is_window: true };
    let mut s = session(vec![w]);
    let Outcome::Captured(r) = drag(&mut s, (350.0, 250.0), (400.0, 300.0)) else { panic!("not captured") };
    assert_eq!(r.screen_rectangle, Rect::new(350, 250, 51, 51));
    assert_eq!(r.window_handle, 0);
}

#[test]
fn tiny_drags_capture_nothing_and_right_click_cancels() {
    let mut s = session(Vec::new());
    assert!(matches!(drag(&mut s, (10.0, 10.0), (12.0, 12.0)), Outcome::Continue));
    assert!(!s.selection.is_valid());
    s.pointer_pressed(PointF::new(10.0, 10.0), Button::Right, none(), 1.0);
    assert!(matches!(s.pointer_released(PointF::new(10.0, 10.0), Button::Right, none()), Outcome::Cancelled));
}

#[test]
fn right_click_while_drawing_only_drops_the_region() {
    let mut s = session(Vec::new());
    s.pointer_pressed(PointF::new(10.0, 10.0), Button::Left, none(), 1.0);
    s.pointer_moved(PointF::new(100.0, 100.0), none());
    s.pointer_pressed(PointF::new(100.0, 100.0), Button::Right, none(), 1.0);
    assert!(!s.selection.is_valid());
    assert!(matches!(s.pointer_released(PointF::new(100.0, 100.0), Button::Right, none()), Outcome::Continue));
}

#[test]
fn shift_draws_squares_and_keys_wait_for_the_input_delay() {
    let mut s = session(Vec::new());
    s.pointer_pressed(PointF::new(100.0, 100.0), Button::Left, none(), 1.0);
    s.pointer_moved(PointF::new(160.0, 130.0), Modifiers { shift: true, ..none() });
    assert_eq!(s.selection, RectF::new(100.0, 100.0, 61.0, 61.0));
    s.pointer_released(PointF::new(160.0, 130.0), Button::Left, none());
    let mut s = session(Vec::new());
    let mut moved = None;
    assert!(matches!(s.key_down(Key::Space, none(), &mut |x, y| moved = Some((x, y))), Outcome::Continue));
    s.tick(Instant::now() + Duration::from_secs(1));
    let Outcome::Captured(r) = s.key_down(Key::Space, none(), &mut |_, _| {}) else { panic!("space didn't capture") };
    assert_eq!(r.screen_rectangle, Rect::new(0, 0, 1024, 768));
    assert!(r.window_info.is_none());
}

#[test]
fn escape_cancels_even_before_keys_work() {
    let mut s = session(Vec::new());
    assert!(matches!(s.key_up(Key::Escape, none()), Outcome::Cancelled));
}

#[test]
fn x_buttons_capture_fullscreen_and_the_active_monitor() {
    let mut s = session(Vec::new());
    let Outcome::Captured(r) = s.pointer_pressed(PointF::new(5.0, 5.0), Button::X2, none(), 1.0) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(0, 0, 1024, 768));
}

#[test]
fn moving_a_region_without_quick_capture() {
    let mut s = session(Vec::new());
    s.options.QuickCapture = false;
    assert!(matches!(drag(&mut s, (100.0, 100.0), (199.0, 149.0)), Outcome::Continue));
    assert_eq!(s.selection, RectF::new(100.0, 100.0, 100.0, 50.0));
    assert!(s.resize_nodes_visible());
    // Drag the bottom-right node out by 10.
    assert!(matches!(drag(&mut s, (200.0, 150.0), (210.0, 160.0)), Outcome::Continue));
    assert_eq!(s.selection, RectF::new(100.0, 100.0, 110.0, 60.0));
    // Move it.
    drag(&mut s, (150.0, 120.0), (160.0, 130.0));
    assert_eq!(s.selection, RectF::new(110.0, 110.0, 110.0, 60.0));
    let Outcome::Captured(r) = s.key_down(Key::Enter, none(), &mut |_, _| {}) else {
        // Keys wait for the input delay.
        s.tick(Instant::now() + Duration::from_secs(1));
        let Outcome::Captured(r) = s.key_down(Key::Enter, none(), &mut |_, _| {}) else { panic!() };
        assert_eq!(r.screen_rectangle, Rect::new(110, 110, 110, 60));
        return;
    };
    assert_eq!(r.screen_rectangle, Rect::new(110, 110, 110, 60));
}

#[test]
fn toolbar_sits_at_the_top_centre_of_the_pointers_monitor() {
    let s = session(Vec::new());
    assert_eq!(s.toolbar_anchor(), Some((512.0, 0.0)));
}

#[test]
fn wheel_resizes_and_hides_the_magnifier() {
    let mut s = session(Vec::new());
    s.wheel(1.0, none());
    assert_eq!((s.options.MagnifierSize, s.options.MagnifierPixelCount), (170, 17));
    s.wheel(1.0, Modifiers { ctrl: true, ..none() });
    assert_eq!((s.options.MagnifierSize, s.options.MagnifierPixelCount), (170, 15));
    s.options.MagnifierSize = 50;
    s.options.MagnifierPixelCount = 5;
    s.wheel(-1.0, none());
    assert!(s.magnifier_hidden_by_wheel);
    s.wheel(1.0, none());
    assert!(!s.magnifier_hidden_by_wheel);
}

fn click_tool(s: &mut RegionCapture, id: &str) {
    let i = s.toolbar_index(id).unwrap();
    s.set_toolbar_hover(Some(toolbar::Part::Tool(i)));
    s.pointer_pressed(PointF::new(0.0, 0.0), Button::Left, none(), 1.0);
    s.pointer_released(PointF::new(0.0, 0.0), Button::Left, none());
    s.set_toolbar_hover(None);
}

#[test]
fn annotations_are_drawn_and_captured_with_the_region() {
    let mut s = session(Vec::new());
    click_tool(&mut s, "Rectangle");
    assert!(!s.region_tool_active);
    assert_eq!(s.active_annotation_tool(), Some(EditorTool::Rectangle));
    // Drawing doesn't capture; it makes an annotation.
    assert!(matches!(drag(&mut s, (100.0, 100.0), (300.0, 200.0)), Outcome::Continue));
    let e = s.editor.as_ref().unwrap();
    assert_eq!(e.annotations().len(), 1);
    let expected = sharex_image_editor::render(e.source(), e.annotations());
    // Tab: the region tool, then a region over it.
    s.tick(Instant::now() + Duration::from_secs(1));
    s.key_down(Key::Tab, none(), &mut |_, _| {});
    assert!(s.region_tool_active);
    let Outcome::Captured(r) = drag(&mut s, (50.0, 50.0), (400.0, 300.0)) else { panic!("not captured") };
    assert!(r.image_modified);
    let (w, h) = r.image.dimensions();
    for (x, y) in [(50, 50), (52, 52), (100 - 50, 100 - 50), (w - 1, h - 1)] {
        let c = expected.pixel(50 + x as i32, 50 + y as i32);
        let want = image::Rgba([(c >> 16) as u8, (c >> 8) as u8, c as u8, (c >> 24) as u8]);
        assert_eq!(r.image.get_pixel(x, y), &want, "at {x},{y}");
    }
}

#[test]
fn right_click_removes_the_tools_annotation_then_cancels() {
    let mut s = session(Vec::new());
    click_tool(&mut s, "Ellipse");
    drag(&mut s, (100.0, 100.0), (300.0, 200.0));
    s.pointer_pressed(PointF::new(200.0, 150.0), Button::Right, none(), 1.0);
    assert!(matches!(s.pointer_released(PointF::new(200.0, 150.0), Button::Right, none()), Outcome::Continue));
    assert!(s.editor.as_ref().unwrap().annotations().is_empty());
    s.pointer_pressed(PointF::new(200.0, 150.0), Button::Right, none(), 1.0);
    assert!(matches!(s.pointer_released(PointF::new(200.0, 150.0), Button::Right, none()), Outcome::Cancelled));
}

#[test]
fn tool_hotkeys_switch_to_the_annotation_tools() {
    let mut s = session(Vec::new());
    s.tick(Instant::now() + Duration::from_secs(1));
    s.key_down(Key::Letter('A'), none(), &mut |_, _| {});
    assert_eq!(s.active_annotation_tool(), Some(EditorTool::Arrow));
    s.key_down(Key::NumPad(1), none(), &mut |_, _| {});
    assert_eq!(s.active_annotation_tool(), Some(EditorTool::Rectangle));
    // Escape with nothing selected cancels.
    assert!(matches!(s.key_up(Key::Escape, none()), Outcome::Cancelled));
}
