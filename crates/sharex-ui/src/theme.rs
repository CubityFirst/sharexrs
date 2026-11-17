//! ShareX's colours and spacing.
//!
//! Colours port `ShareX.Avalonia/Theming/ShareXTheme.axaml`; spacing ports
//! the `ShareX.Density.*` keys of `ControlStyles.axaml` (the default, 100%)
//! and `CompactMode.axaml` (0%), blended as `ThemeManager.InterpolateSpacing`
//! does.

use egui::{Color32, CornerRadius, Margin, Stroke, Theme, Visuals};

const fn hex(rgb: u32) -> Color32 {
    Color32::from_rgb((rgb >> 16) as u8, (rgb >> 8) as u8, rgb as u8)
}

/// `#AARRGGBB`, as XAML writes translucent colours.
const fn hexa(argb: u32) -> Color32 {
    let a = argb >> 24;
    Color32::from_rgba_premultiplied(pm(argb >> 16, a), pm(argb >> 8, a), pm(argb, a), a as u8)
}

const fn pm(c: u32, a: u32) -> u8 {
    ((c & 0xFF) * a / 255) as u8
}

/// One theme's `ShareX.Color.*` resources.
#[derive(Clone, Debug)]
pub struct Palette {
    pub accent_start: Color32,
    pub accent_end: Color32,
    pub accent_foreground: Color32,
    pub accent_text: Color32,
    pub accent_border: Color32,
    pub accent_icon: Color32,
    pub link: Color32,
    pub text: Color32,
    pub text_secondary: Color32,
    pub success: Color32,
    pub error: Color32,
    pub background_main: Color32,
    pub background_panel: Color32,
    pub background_popup: Color32,
    pub background_menu: Color32,
    pub background_toolbar: Color32,
    pub border: Color32,
    pub control_background: Color32,
    pub control_background_hover: Color32,
    pub control_border: Color32,
    pub control_background_disabled: Color32,
    pub separator: Color32,
    pub cancel_background: Color32,
    pub cancel_border: Color32,
    pub continue_background: Color32,
    pub continue_border: Color32,
    pub task_mode_foreground: Color32,
    pub overlay_soft: Color32,
    pub overlay_modal: Color32,
    /// `ShareX.Brush.Checkered`: the base colour and the alternate squares.
    pub checkered: [Color32; 2],
}

/// "ShareX Dark".
pub const DARK: Palette = Palette {
    accent_start: hex(0x09AAFF),
    accent_end: hex(0x0899E6),
    accent_foreground: hex(0x111111),
    accent_text: hex(0x0EACFF),
    accent_border: hex(0x09AAFF),
    accent_icon: hex(0x09AAFF),
    link: hex(0x0EACFF),
    text: hex(0xD8DADB),
    text_secondary: hex(0xD8DADB),
    success: hex(0x79B58A),
    error: hex(0xD07C7C),
    background_main: hex(0x272727),
    background_panel: hex(0x242424),
    background_popup: hex(0x2E2E2E),
    background_menu: hex(0x2B2B2B),
    background_toolbar: hex(0x222222),
    border: hex(0x1F1F1F),
    control_background: hex(0x2E2E2E),
    control_background_hover: hex(0x3A3A3A),
    control_border: hex(0x1F1F1F),
    control_background_disabled: hex(0x383838),
    separator: hex(0x333333),
    cancel_background: hex(0xDC2626),
    cancel_border: hex(0xEF4444),
    continue_background: hex(0x10B981),
    continue_border: hex(0x13DD9A),
    task_mode_foreground: hex(0xFFFFFF),
    overlay_soft: hexa(0x30000000),
    overlay_modal: hexa(0x80000000),
    checkered: [hex(0x272727), hex(0x2E2E2E)],
};

/// "ShareX Light".
pub const LIGHT: Palette = Palette {
    accent_start: hex(0x09AAFF),
    accent_end: hex(0x0899E6),
    accent_foreground: hex(0x111111),
    accent_text: hex(0x0670A8),
    accent_border: hex(0x078DD4),
    accent_icon: hex(0x09AAFF),
    link: hex(0x0670A8),
    text: hex(0x4E4E4E),
    text_secondary: hex(0x4E4E4E),
    success: hex(0x4F8A63),
    error: hex(0xB85F66),
    background_main: hex(0xF5F5F7),
    background_panel: hex(0xFDFDFD),
    background_popup: hex(0xFDFDFD),
    background_menu: hex(0xF2F2F2),
    background_toolbar: hex(0xFDFDFD),
    border: hex(0xE5E5E8),
    control_background: hex(0xF5F5F7),
    control_background_hover: hex(0xEBEBEF),
    control_border: hex(0xDBDBDF),
    control_background_disabled: hex(0xAEAEB2),
    separator: hex(0xE3E3E3),
    cancel_background: hex(0xDC2626),
    cancel_border: hex(0xEF4444),
    continue_background: hex(0x10B981),
    continue_border: hex(0x13DD9A),
    task_mode_foreground: hex(0xFFFFFF),
    overlay_soft: hexa(0x30000000),
    overlay_modal: hexa(0x80000000),
    checkered: [hex(0xFFFFFF), hex(0xEBEBEB)],
};

/// The Avalonia Fluent theme's own colours, which show wherever ShareX
/// leaves a Fluent control template's brush alone (a menu item's chevron,
/// its disabled text, the item whose submenu is open, a submenu's border).
/// From `Avalonia.Themes.Fluent/Accents/BaseColorsPalette.xaml` and
/// `BaseResources.xaml` (Avalonia 12.1.6, the version ShareX builds with).
#[derive(Clone, Debug)]
pub struct Fluent {
    /// `SystemBaseHighColor`: `SystemControlHighlightAltBaseHighBrush`.
    pub base_high: Color32,
    /// `SystemBaseMediumHighColor`: `MenuFlyoutSubItemChevron`.
    pub base_medium_high: Color32,
    /// `SystemBaseMediumColor`: `MenuFlyoutItemKeyboardAcceleratorTextForeground`.
    pub base_medium: Color32,
    /// `SystemBaseMediumLowColor`: `SystemControlDisabledBaseMediumLowBrush`.
    pub base_medium_low: Color32,
    /// `SystemListLowColor`: `MenuFlyoutItemBackgroundPointerOver`.
    pub list_low: Color32,
    /// `SystemListMediumColor`: `MenuFlyoutItemBackgroundPressed`.
    pub list_medium: Color32,
    /// `SystemControlTransientBorderBrush` (`#000000` at 36% / 14%):
    /// `MenuFlyoutPresenterBorderBrush`, a submenu's border.
    pub transient_border: Color32,
}

pub const FLUENT_DARK: Fluent = Fluent {
    base_high: hexa(0xFFFFFFFF),
    base_medium_high: hexa(0xCCFFFFFF),
    base_medium: hexa(0x99FFFFFF),
    base_medium_low: hexa(0x66FFFFFF),
    list_low: hexa(0x19FFFFFF),
    list_medium: hexa(0x33FFFFFF),
    transient_border: hexa(0x5C000000),
};

pub const FLUENT_LIGHT: Fluent = Fluent {
    base_high: hexa(0xFF000000),
    base_medium_high: hexa(0xCC000000),
    base_medium: hexa(0x99000000),
    base_medium_low: hexa(0x66000000),
    list_low: hexa(0x19000000),
    list_medium: hexa(0x33000000),
    transient_border: hexa(0x24000000),
};

/// The Fluent colours of the theme `ctx` is showing.
pub fn fluent(ctx: &egui::Context) -> &'static Fluent {
    match ctx.theme() {
        Theme::Dark => &FLUENT_DARK,
        Theme::Light => &FLUENT_LIGHT,
    }
}

/// The red ShareX uses for failed hotkeys and tasks (`#C65F5F`, written
/// inline in `MainWindow.axaml` rather than as a resource).
pub const FAILED: Color32 = hex(0xC65F5F);

/// The palette of the theme `ctx` is showing.
pub fn palette(ctx: &egui::Context) -> &'static Palette {
    match ctx.theme() {
        Theme::Dark => &DARK,
        Theme::Light => &LIGHT,
    }
}

/// `ShareX.FontSize.Default`.
pub const FONT_SIZE: f32 = 13.0;

/// How roomy each area is, as Application settings › Theme's spacing
/// sliders set it: 0 is compact, 100 the default, 200 the most
/// (`ApplicationThemeOptions.*Spacing`).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Spacing {
    pub sidebar: f32,
    pub menus: f32,
    pub tool_menus: f32,
    pub settings: f32,
}

impl Default for Spacing {
    fn default() -> Self {
        Self { sidebar: 100.0, menus: 100.0, tool_menus: 100.0, settings: 100.0 }
    }
}

/// A XAML `Thickness`: left, top, right, bottom.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Thickness {
    pub left: f32,
    pub top: f32,
    pub right: f32,
    pub bottom: f32,
}

impl Thickness {
    pub const fn new(left: f32, top: f32, right: f32, bottom: f32) -> Self {
        Self { left, top, right, bottom }
    }
    /// XAML's two-value form, `"h,v"`.
    pub const fn hv(h: f32, v: f32) -> Self {
        Self::new(h, v, h, v)
    }
    pub const fn all(v: f32) -> Self {
        Self::new(v, v, v, v)
    }
    pub fn margin(self) -> Margin {
        Margin {
            left: self.left as i8,
            right: self.right as i8,
            top: self.top as i8,
            bottom: self.bottom as i8,
        }
    }
    fn lerp(compact: Self, default: Self, t: f32) -> Self {
        Self::new(
            whole(compact.left, default.left, t),
            whole(compact.top, default.top, t),
            whole(compact.right, default.right, t),
            whole(compact.bottom, default.bottom, t),
        )
    }
}

/// Every `ShareX.Density.*` value, blended for the chosen [`Spacing`].
#[derive(Clone, Debug, PartialEq)]
pub struct Density {
    pub nav_min_width: f32,
    pub nav_panel_margin: Thickness,
    pub nav_button_height: f32,
    pub nav_button_padding: Thickness,
    pub nav_label_margin: Thickness,
    pub menu_padding: Thickness,
    pub menu_item_height: f32,
    pub menu_item_padding: Thickness,
    pub menu_separator_margin: Thickness,
    /// `MenuFlyoutPresenterThemePadding`: a submenu popup's padding.
    pub menu_flyout_presenter_padding: Thickness,
    /// `MenuIconPresenterMargin` (Fluent's `MenuItem.xaml`): after a menu
    /// item's toggle and icon columns.
    pub menu_icon_presenter_margin: Thickness,
    pub tool_grid_padding: Thickness,
    pub tool_grid_button_height: f32,
    pub tool_grid_button_padding: Thickness,
    pub tool_grid_item_spacing: f32,
    pub tool_grid_header_font_size: f32,
    pub tool_grid_header_margin: Thickness,
    pub tool_grid_first_header_margin: Thickness,
    pub tool_grid_separator_margin: Thickness,
    pub settings_page_margin: Thickness,
    pub settings_page_title_margin: Thickness,
    pub settings_page_title_font_size: f32,
    pub settings_page_title_icon_size: f32,
    pub settings_section_title_font_size: f32,
    pub settings_card_padding: Thickness,
    pub settings_card_margin: Thickness,
    pub settings_check_box_min_height: f32,
    pub settings_control_min_height: f32,
    pub settings_control_padding: Thickness,
    pub settings_number_box_height: f32,
    pub settings_button_padding: Thickness,
    pub settings_nav_item_min_height: f32,
    pub settings_nav_item_margin: Thickness,
    pub settings_search_padding: Thickness,
    pub settings_nav_spacing: f32,
    pub settings_window_width: f32,
}

/// `ControlStyles.axaml`'s defaults (100%).
const DEFAULT: Density = Density {
    nav_min_width: 210.0,
    nav_panel_margin: Thickness::new(6.0, 6.0, 6.0, 0.0),
    nav_button_height: 30.0,
    nav_button_padding: Thickness::hv(8.0, 2.0),
    nav_label_margin: Thickness::new(7.0, 0.0, 0.0, 0.0),
    menu_padding: Thickness::hv(6.0, 2.0),
    menu_item_height: 28.0,
    menu_item_padding: Thickness::hv(7.0, 2.0),
    menu_separator_margin: Thickness::hv(6.0, 2.0),
    menu_flyout_presenter_padding: Thickness::hv(6.0, 2.0),
    menu_icon_presenter_margin: Thickness::new(0.0, 0.0, 12.0, 0.0),
    tool_grid_padding: Thickness::all(10.0),
    tool_grid_button_height: 34.0,
    tool_grid_button_padding: Thickness::hv(8.0, 4.0),
    tool_grid_item_spacing: 8.0,
    tool_grid_header_font_size: 16.0,
    tool_grid_header_margin: Thickness::new(7.0, 3.0, 7.0, 3.0),
    tool_grid_first_header_margin: Thickness::new(7.0, 1.0, 7.0, 3.0),
    tool_grid_separator_margin: Thickness::new(7.0, 9.0, 7.0, 7.0),
    settings_page_margin: Thickness::new(28.0, 24.0, 28.0, 32.0),
    settings_page_title_margin: Thickness::new(0.0, 0.0, 0.0, 14.0),
    settings_page_title_font_size: 24.0,
    settings_page_title_icon_size: 22.0,
    settings_section_title_font_size: 15.0,
    settings_card_padding: Thickness::all(12.0),
    settings_card_margin: Thickness::new(0.0, 0.0, 0.0, 8.0),
    settings_check_box_min_height: 32.0,
    settings_control_min_height: 32.0,
    settings_control_padding: Thickness::hv(8.0, 4.0),
    settings_number_box_height: 40.0,
    settings_button_padding: Thickness::hv(10.0, 4.0),
    settings_nav_item_min_height: 32.0,
    settings_nav_item_margin: Thickness::hv(2.0, 3.0),
    settings_search_padding: Thickness::new(30.0, 8.0, 8.0, 8.0),
    settings_nav_spacing: 10.0,
    settings_window_width: 950.0,
};

/// `CompactMode.axaml` (0%).
const COMPACT: Density = Density {
    nav_min_width: 180.0,
    nav_panel_margin: Thickness::new(4.0, 4.0, 4.0, 0.0),
    nav_button_height: 24.0,
    nav_button_padding: Thickness::hv(6.0, 0.0),
    nav_label_margin: Thickness::new(5.0, 0.0, 0.0, 0.0),
    menu_padding: Thickness::all(2.0),
    menu_item_height: 22.0,
    menu_item_padding: Thickness::hv(5.0, 0.0),
    menu_separator_margin: Thickness::hv(4.0, 2.0),
    menu_flyout_presenter_padding: Thickness::all(2.0),
    menu_icon_presenter_margin: Thickness::new(0.0, 0.0, 8.0, 0.0),
    tool_grid_padding: Thickness::all(6.0),
    tool_grid_button_height: 26.0,
    tool_grid_button_padding: Thickness::hv(6.0, 2.0),
    tool_grid_item_spacing: 6.0,
    tool_grid_header_font_size: 14.0,
    tool_grid_header_margin: Thickness::new(7.0, 1.0, 7.0, 1.0),
    tool_grid_first_header_margin: Thickness::new(7.0, 0.0, 7.0, 1.0),
    tool_grid_separator_margin: Thickness::new(7.0, 5.0, 7.0, 4.0),
    settings_page_margin: Thickness::new(18.0, 12.0, 18.0, 16.0),
    settings_page_title_margin: Thickness::new(0.0, 0.0, 0.0, 8.0),
    settings_page_title_font_size: 19.0,
    settings_page_title_icon_size: 18.0,
    settings_section_title_font_size: 14.0,
    settings_card_padding: Thickness::hv(10.0, 6.0),
    settings_card_margin: Thickness::new(0.0, 0.0, 0.0, 6.0),
    settings_check_box_min_height: 24.0,
    settings_control_min_height: 26.0,
    settings_control_padding: Thickness::hv(6.0, 2.0),
    settings_number_box_height: 26.0,
    settings_button_padding: Thickness::hv(8.0, 2.0),
    settings_nav_item_min_height: 24.0,
    settings_nav_item_margin: Thickness::hv(2.0, 1.0),
    settings_search_padding: Thickness::new(30.0, 4.0, 8.0, 4.0),
    settings_nav_spacing: 6.0,
    settings_window_width: 840.0,
};

/// `ThemeManager.InterpolateSpacing`'s lerp for whole-pixel values.
fn whole(compact: f32, default: f32, t: f32) -> f32 {
    (compact + (default - compact) * t).max(0.0).round()
}

/// The same for text and icon sizes, which move in half-pixel steps.
fn text(compact: f32, default: f32, t: f32) -> f32 {
    ((compact + (default - compact) * t).max(0.0) * 2.0).round() / 2.0
}

impl Default for Density {
    fn default() -> Self {
        DEFAULT
    }
}

impl Density {
    pub fn new(s: Spacing) -> Self {
        let (c, d) = (&COMPACT, &DEFAULT);
        let (nav, menu, tool, set) =
            (s.sidebar / 100.0, s.menus / 100.0, s.tool_menus / 100.0, s.settings / 100.0);
        let t = Thickness::lerp;
        Self {
            nav_min_width: whole(c.nav_min_width, d.nav_min_width, nav),
            nav_panel_margin: t(c.nav_panel_margin, d.nav_panel_margin, nav),
            nav_button_height: whole(c.nav_button_height, d.nav_button_height, nav),
            nav_button_padding: t(c.nav_button_padding, d.nav_button_padding, nav),
            nav_label_margin: t(c.nav_label_margin, d.nav_label_margin, nav),
            menu_padding: t(c.menu_padding, d.menu_padding, menu),
            menu_item_height: whole(c.menu_item_height, d.menu_item_height, menu),
            menu_item_padding: t(c.menu_item_padding, d.menu_item_padding, menu),
            menu_separator_margin: t(c.menu_separator_margin, d.menu_separator_margin, menu),
            menu_flyout_presenter_padding: t(
                c.menu_flyout_presenter_padding,
                d.menu_flyout_presenter_padding,
                menu,
            ),
            menu_icon_presenter_margin: t(c.menu_icon_presenter_margin, d.menu_icon_presenter_margin, menu),
            tool_grid_padding: t(c.tool_grid_padding, d.tool_grid_padding, tool),
            tool_grid_button_height: whole(c.tool_grid_button_height, d.tool_grid_button_height, tool),
            tool_grid_button_padding: t(c.tool_grid_button_padding, d.tool_grid_button_padding, tool),
            tool_grid_item_spacing: whole(c.tool_grid_item_spacing, d.tool_grid_item_spacing, tool),
            tool_grid_header_font_size: text(c.tool_grid_header_font_size, d.tool_grid_header_font_size, tool),
            tool_grid_header_margin: t(c.tool_grid_header_margin, d.tool_grid_header_margin, tool),
            tool_grid_first_header_margin: t(c.tool_grid_first_header_margin, d.tool_grid_first_header_margin, tool),
            tool_grid_separator_margin: t(c.tool_grid_separator_margin, d.tool_grid_separator_margin, tool),
            settings_page_margin: t(c.settings_page_margin, d.settings_page_margin, set),
            settings_page_title_margin: t(c.settings_page_title_margin, d.settings_page_title_margin, set),
            settings_page_title_font_size: text(c.settings_page_title_font_size, d.settings_page_title_font_size, set),
            settings_page_title_icon_size: text(c.settings_page_title_icon_size, d.settings_page_title_icon_size, set),
            settings_section_title_font_size: text(
                c.settings_section_title_font_size,
                d.settings_section_title_font_size,
                set,
            ),
            settings_card_padding: t(c.settings_card_padding, d.settings_card_padding, set),
            settings_card_margin: t(c.settings_card_margin, d.settings_card_margin, set),
            settings_check_box_min_height: whole(c.settings_check_box_min_height, d.settings_check_box_min_height, set),
            settings_control_min_height: whole(c.settings_control_min_height, d.settings_control_min_height, set),
            settings_control_padding: t(c.settings_control_padding, d.settings_control_padding, set),
            settings_number_box_height: whole(c.settings_number_box_height, d.settings_number_box_height, set),
            settings_button_padding: t(c.settings_button_padding, d.settings_button_padding, set),
            settings_nav_item_min_height: whole(c.settings_nav_item_min_height, d.settings_nav_item_min_height, set),
            settings_nav_item_margin: t(c.settings_nav_item_margin, d.settings_nav_item_margin, set),
            settings_search_padding: t(c.settings_search_padding, d.settings_search_padding, set),
            settings_nav_spacing: whole(c.settings_nav_spacing, d.settings_nav_spacing, set),
            settings_window_width: whole(c.settings_window_width, d.settings_window_width, set),
        }
    }
}

/// The current [`Density`], kept in the context's memory so every widget
/// can read it.
pub fn density(ctx: &egui::Context) -> Density {
    ctx.data(|d| d.get_temp::<Density>(egui::Id::NULL)).unwrap_or_default()
}

pub fn set_density(ctx: &egui::Context, density: Density) {
    ctx.data_mut(|d| d.insert_temp(egui::Id::NULL, density));
}

/// Puts ShareX's palettes, base font size and spacing on `ctx`.
pub fn apply(ctx: &egui::Context, spacing_percent: f32) {
    ctx.set_visuals_of(Theme::Dark, visuals(Visuals::dark(), &DARK));
    ctx.set_visuals_of(Theme::Light, visuals(Visuals::light(), &LIGHT));
    ctx.all_styles_mut(|style| {
        use egui::{FontFamily, FontId, TextStyle};
        style.text_styles = [
            (TextStyle::Small, FontId::new(11.0, FontFamily::Proportional)),
            (TextStyle::Body, FontId::new(FONT_SIZE, FontFamily::Proportional)),
            (TextStyle::Button, FontId::new(FONT_SIZE, FontFamily::Proportional)),
            (TextStyle::Heading, FontId::new(19.0, crate::fonts::semibold())),
            (TextStyle::Monospace, FontId::new(FONT_SIZE, FontFamily::Monospace)),
        ]
        .into();
        style.spacing.item_spacing = egui::vec2(0.0, 0.0);
        style.spacing.button_padding = egui::vec2(8.0, 4.0);
        style.interaction.selectable_labels = false;
        // Glyphs on whole pixels on macOS: at 2x, quarter-pixel offsets come
        // out soft (snapr b25a53e). Reset by every set_visuals, so here.
        if cfg!(target_os = "macos") {
            style.visuals.text_options.subpixel_binning = false;
        }
    });
    let pct = spacing_percent;
    set_density(ctx, Density::new(Spacing { sidebar: pct, menus: pct, tool_menus: pct, settings: pct }));
}

fn visuals(mut v: Visuals, p: &Palette) -> Visuals {
    v.panel_fill = p.background_main;
    v.window_fill = p.background_popup;
    v.extreme_bg_color = p.control_background;
    v.faint_bg_color = p.background_panel;
    v.window_stroke = Stroke::new(1.0, p.border);
    v.override_text_color = Some(p.text);
    v.hyperlink_color = p.link;
    v.selection.bg_fill = p.accent_start;
    v.selection.stroke = Stroke::new(1.0, p.accent_foreground);
    v.window_corner_radius = CornerRadius::same(8);
    v.menu_corner_radius = CornerRadius::same(8);
    let w = &mut v.widgets;
    for (state, fill) in [
        (&mut w.noninteractive, p.background_main),
        (&mut w.inactive, p.control_background),
        (&mut w.hovered, p.control_background_hover),
        (&mut w.active, p.control_background),
        (&mut w.open, p.control_background_hover),
    ] {
        state.bg_fill = fill;
        state.weak_bg_fill = fill;
        state.bg_stroke = Stroke::new(1.0, p.control_border);
        state.fg_stroke = Stroke::new(1.0, p.text);
        state.corner_radius = CornerRadius::same(4);
        state.expansion = 0.0;
    }
    w.noninteractive.bg_stroke = Stroke::new(1.0, p.separator);
    v
}
