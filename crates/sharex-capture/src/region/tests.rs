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
        allow_multiple_regions: true,
        pixel_info: None,
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
    assert!(!s.selection.bounds.is_valid());
    s.pointer_pressed(PointF::new(10.0, 10.0), Button::Right, none(), 1.0);
    assert!(matches!(s.pointer_released(PointF::new(10.0, 10.0), Button::Right, none()), Outcome::Cancelled));
}

#[test]
fn right_click_while_drawing_only_drops_the_region() {
    let mut s = session(Vec::new());
    s.pointer_pressed(PointF::new(10.0, 10.0), Button::Left, none(), 1.0);
    s.pointer_moved(PointF::new(100.0, 100.0), none());
    s.pointer_pressed(PointF::new(100.0, 100.0), Button::Right, none(), 1.0);
    assert!(!s.selection.bounds.is_valid());
    assert!(matches!(s.pointer_released(PointF::new(100.0, 100.0), Button::Right, none()), Outcome::Continue));
}

#[test]
fn shift_draws_squares_and_keys_wait_for_the_input_delay() {
    let mut s = session(Vec::new());
    s.pointer_pressed(PointF::new(100.0, 100.0), Button::Left, none(), 1.0);
    s.pointer_moved(PointF::new(160.0, 130.0), Modifiers { shift: true, ..none() });
    assert_eq!(s.selection.bounds, RectF::new(100.0, 100.0, 61.0, 61.0));
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
    assert_eq!(s.selection.bounds, RectF::new(100.0, 100.0, 100.0, 50.0));
    assert!(s.resize_nodes_visible());
    // Drag the bottom-right node out by 10.
    assert!(matches!(drag(&mut s, (200.0, 150.0), (210.0, 160.0)), Outcome::Continue));
    assert_eq!(s.selection.bounds, RectF::new(100.0, 100.0, 110.0, 60.0));
    // Move it.
    drag(&mut s, (150.0, 120.0), (160.0, 130.0));
    assert_eq!(s.selection.bounds, RectF::new(110.0, 110.0, 110.0, 60.0));
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

#[test]
fn the_emoji_tool_asks_the_host_for_its_picker_and_a_picked_colour_sets_the_option() {
    let mut s = session(Vec::new());
    s.tick(Instant::now() + Duration::from_secs(1));
    click_tool(&mut s, "Emoji");
    let reqs = s.take_editor_requests();
    assert!(reqs.iter().any(|r| matches!(r, ed::Request::EmojiPicker)), "{reqs:?}");
    s.insert_emoji("1F600", "grinning face", (10.0, 10.0));
    click_tool(&mut s, "Rectangle");
    // Another tool doesn't ask for the picker again.
    let reqs = s.take_editor_requests();
    assert!(reqs.is_empty(), "{reqs:?}");
    assert_eq!(s.active_annotation_tool(), Some(EditorTool::Rectangle));
    s.set_picked_color(super::options_row::OptionItem::BorderColor, [1, 2, 3, 255]);
    assert_eq!(s.editor.as_ref().unwrap().tools.options.BorderColorHex.to_ascii_uppercase(), "#FF010203");
}

#[test]
fn hidden_and_reordered_items_change_the_capture_toolbar() {
    let saved = vec![
        serde_json::json!({"Id": "RegionEllipse", "IsVisible": true}),
        serde_json::json!({"Id": "Rectangle", "IsVisible": false}),
    ];
    let items = toolbar::items(&saved);
    assert_eq!(items[0].def.id, "RegionEllipse");
    assert!(!items.iter().any(|i| i.def.id == "Rectangle"));
    assert!(!items.iter().any(|i| i.def.id == "RegionFreehand"));
}

fn session_with(options: RegionCaptureOptions) -> RegionCapture {
    let mut s = session(Vec::new());
    s.options = options;
    s.bar = toolbar::items(&s.options.ToolbarItems);
    s
}

fn shapes_visible() -> RegionCaptureOptions {
    RegionCaptureOptions {
        ToolbarItems: vec![
            serde_json::json!({"Id": "RegionEllipse", "IsVisible": true}),
            serde_json::json!({"Id": "RegionFreehand", "IsVisible": true}),
        ],
        ..Default::default()
    }
}

#[test]
fn the_ellipse_region_captures_its_bounds_with_the_corners_transparent() {
    let mut s = session_with(shapes_visible());
    click_tool(&mut s, "RegionEllipse");
    assert!(s.region_tool_active);
    assert_eq!(s.region_shape, RegionShape::Ellipse);
    let Outcome::Captured(r) = drag(&mut s, (100.0, 100.0), (299.0, 199.0)) else { panic!("not captured") };
    assert_eq!(r.screen_rectangle, Rect::new(100, 100, 200, 100));
    assert_eq!(r.image.get_pixel(0, 0)[3], 0);
    assert_eq!(r.image.get_pixel(100, 50), &image::Rgba([200, 150, 70, 255]));
    assert_eq!(r.window_handle, 0);
}

#[test]
fn the_freehand_region_follows_the_pointer_and_shift_draws_a_straight_edge() {
    let mut s = session_with(shapes_visible());
    click_tool(&mut s, "RegionFreehand");
    s.options.QuickCapture = false;
    s.pointer_pressed(PointF::new(100.0, 100.0), Button::Left, none(), 1.0);
    s.pointer_moved(PointF::new(200.0, 100.0), none());
    let shift = Modifiers { shift: true, ..none() };
    s.pointer_moved(PointF::new(200.0, 150.0), shift);
    s.pointer_moved(PointF::new(200.0, 200.0), shift);
    s.pointer_moved(PointF::new(100.0, 200.0), none());
    s.pointer_released(PointF::new(100.0, 200.0), Button::Left, none());
    let pts = s.selection.points.clone().unwrap();
    assert_eq!(pts, vec![
        PointF::new(100.0, 100.0),
        PointF::new(200.0, 100.0),
        PointF::new(200.0, 200.0),
        PointF::new(100.0, 200.0)
    ]);
    // A freehand region moves but isn't resized.
    assert!(!s.resize_nodes_visible());
    s.tick(Instant::now() + Duration::from_secs(1));
    let Outcome::Captured(r) = s.key_down(Key::Enter, none(), &mut |_, _| {}) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(100, 100, 100, 100));
}

#[test]
fn multi_region_mode_keeps_regions_and_captures_them_masked() {
    let mut s = session_with(RegionCaptureOptions { MultiRegionMode: true, ..Default::default() });
    assert!(!s.quick_capture());
    drag(&mut s, (10.0, 10.0), (59.0, 59.0));
    drag(&mut s, (100.0, 100.0), (149.0, 149.0));
    assert_eq!(s.regions.len(), 1);
    assert!(s.has_valid_selection());
    // Pressing the first region makes it the selection again.
    s.pointer_moved(PointF::new(20.0, 20.0), none());
    s.pointer_pressed(PointF::new(20.0, 20.0), Button::Left, none(), 1.0);
    s.pointer_released(PointF::new(20.0, 20.0), Button::Left, none());
    assert_eq!(s.selection.bounds, RectF::new(10.0, 10.0, 50.0, 50.0));
    assert_eq!(s.regions[0].bounds, RectF::new(100.0, 100.0, 50.0, 50.0));
    s.tick(Instant::now() + Duration::from_secs(1));
    let Outcome::Captured(r) = s.key_down(Key::Enter, none(), &mut |_, _| {}) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(10, 10, 140, 140));
    assert_eq!(r.regions.len(), 2);
    assert_eq!(r.image.get_pixel(70, 10)[3], 0);
    assert_eq!(r.image.get_pixel(5, 5)[3], 255);
}

#[test]
fn delete_removes_the_selection_then_the_last_region() {
    let mut s = session_with(RegionCaptureOptions { MultiRegionMode: true, ..Default::default() });
    s.tick(Instant::now() + Duration::from_secs(1));
    drag(&mut s, (10.0, 10.0), (59.0, 59.0));
    drag(&mut s, (100.0, 100.0), (149.0, 149.0));
    s.key_down(Key::Delete, none(), &mut |_, _| {});
    assert!(!s.has_valid_selection());
    assert_eq!(s.regions.len(), 1);
    s.key_down(Key::Delete, none(), &mut |_, _| {});
    assert!(!s.has_any_region());
}

#[test]
fn custom_info_text_shows_the_pixel() {
    let mut s = session(Vec::new());
    assert_eq!(s.pointer_info_text(3, 4), "X: 3 Y: 4");
    s.options.UseCustomInfoText = true;
    s.options.CustomInfoText.clear();
    assert_eq!(s.pointer_info_text(3, 4), "RGB: 3, 4, 70\r\nHex: 030446\r\nX: 3 Y: 4");
}

/// Writes the overlay's capture of a drag over the test pattern to
/// `SHAREXRS_REGION_OUT` (for comparing with ShareX's file).
#[test]
fn region_capture_over_the_pattern_to_a_file() {
    let Ok(out) = std::env::var("SHAREXRS_REGION_OUT") else { return };
    let spec = std::env::var("SHAREXRS_REGION_SPEC").unwrap_or_default();
    let mut shot = RgbaImage::new(1024, 768);
    for (x, y, p) in shot.enumerate_pixels_mut() {
        let (x, y) = (x as i32, y as i32);
        let c = if x % 64 == 0 || y % 64 == 0 {
            [255, 255, 255]
        } else {
            [(x % 256) as u8, (y % 256) as u8, if ((x / 16) + (y / 16)) % 2 == 0 { 200 } else { 70 }]
        };
        *p = image::Rgba([c[0], c[1], c[2], 255]);
    }
    let screen = Screen { bounds: Rect::new(0, 0, 1024, 768), work_area: Rect::new(0, 0, 1024, 720), scale: 1.0, primary: true };
    let mut s = RegionCapture::new(Request {
        screenshot: shot,
        screen_bounds: screen.bounds,
        screens: vec![screen],
        options: shapes_visible(),
        windows: Vec::new(),
        cursor: Some((600, 600)),
        last_regions: Vec::new(),
        enable_annotations: true,
        editor_options: Default::default(),
        cursor_image: None,
        allow_multiple_regions: true,
        pixel_info: None,
    });
    s.bar = toolbar::items(&s.options.ToolbarItems);
    if spec.contains("freehand") {
        click_tool(&mut s, "RegionFreehand");
    } else {
        click_tool(&mut s, "RegionEllipse");
    }
    let mut at = PointF::new(200.0, 200.0);
    s.pointer_moved(at, none());
    s.pointer_pressed(at, Button::Left, none(), 1.0);
    let mut result = None;
    let path: Vec<(f64, f64, bool)> = if spec.contains("freehand") {
        vec![(450.0, 200.0, false), (450.0, 400.0, false), (300.0, 450.0, true), (200.0, 400.0, false)]
    } else {
        vec![(450.0, 400.0, false)]
    };
    for (tx, ty, shift) in path {
        let from = at;
        for i in 1..=12 {
            let t = i as f64 / 12.0;
            at = PointF::new((from.x + (tx - from.x) * t).round_ties_even(), (from.y + (ty - from.y) * t).round_ties_even());
            s.pointer_moved(at, Modifiers { shift, ..none() });
        }
    }
    if let Outcome::Captured(r) = s.pointer_released(at, Button::Left, none()) {
        result = Some(r);
    }
    result.expect("captured").image.save(out).unwrap();
}

/// ShareX #9277: "Capture selected region" captures the selection (and
/// added regions), or the hovered window when nothing is selected, and does
/// nothing when there's neither.
#[test]
fn the_capture_selection_action_captures_the_selection_or_the_hovered_window() {
    let options = || RegionCaptureOptions {
        MultiRegionMode: true,
        RegionCaptureActionX1Click: RegionCaptureAction::CaptureSelection,
        ..Default::default()
    };
    let mut s = session_with(options());
    drag(&mut s, (10.0, 10.0), (59.0, 59.0));
    assert!(s.has_valid_selection());
    let Outcome::Captured(r) = s.pointer_pressed(PointF::new(500.0, 500.0), Button::X1, none(), 1.0) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(10, 10, 50, 50));

    // Nothing selected: the hovered window.
    let w = SimpleWindowInfo { handle: 42, rect: Rect::new(300, 200, 400, 300), is_window: true };
    let mut s = session(vec![w]);
    s.options = options();
    s.pointer_moved(PointF::new(650.0, 420.0), none());
    let Outcome::Captured(r) = s.pointer_pressed(PointF::new(650.0, 420.0), Button::X1, none(), 1.0) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(300, 200, 400, 300));
    assert_eq!(r.window_handle, 42);

    // Nothing selected or hovered: nothing happens.
    let mut s = session_with(options());
    s.hover = RectF::default();
    assert!(matches!(s.run_action(RegionCaptureAction::CaptureSelection), Outcome::Continue));
}

/// ShareX #9277: with an annotation tool active the side buttons run their
/// actions too (`OnCaptureHostPointerPressed`).
#[test]
fn side_buttons_run_their_actions_with_an_annotation_tool_active() {
    let mut s = session_with(RegionCaptureOptions {
        MultiRegionMode: true,
        RegionCaptureActionX2Click: RegionCaptureAction::CaptureSelection,
        ..Default::default()
    });
    drag(&mut s, (10.0, 10.0), (59.0, 59.0));
    s.activate_annotation_tool();
    assert!(!s.region_tool_active);
    let Outcome::Captured(r) = s.pointer_pressed(PointF::new(500.0, 500.0), Button::X2, none(), 1.0) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(10, 10, 50, 50));
    // X1 is Capture fullscreen by default.
    let mut s = session_with(RegionCaptureOptions::default());
    s.activate_annotation_tool();
    let Outcome::Captured(r) = s.pointer_pressed(PointF::new(500.0, 500.0), Button::X1, none(), 1.0) else { panic!() };
    assert_eq!(r.screen_rectangle, Rect::new(0, 0, 1024, 768));
}
