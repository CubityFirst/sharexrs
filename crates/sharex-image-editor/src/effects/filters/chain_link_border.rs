//! `ChainLinkBorderImageEffect` (`Core/ImageEffects/Filters/ChainLinkBorderImageEffect.cs`).

use crate::effects::prelude::*;

/// `ChainLinkBorderImageEffect`.
#[derive(Clone, Debug)]
pub struct ChainLinkBorderImageEffect {
    pub border_size: i32,
    pub wire_color: SkColor,
    pub bg_color: SkColor,
    pub cell_size: f32,
    pub wire_width: f32,
    pub highlight: bool,
}

impl Default for ChainLinkBorderImageEffect {
    fn default() -> Self {
        ChainLinkBorderImageEffect {
            border_size: 52,
            wire_color: SkColor::argb(0xFFAAAFB4),
            bg_color: SkColor::argb(0xFF232830),
            cell_size: 20.0,
            wire_width: 2.5,
            highlight: true,
        }
    }
}

impl EffectImpl for ChainLinkBorderImageEffect {
    fn info() -> Info {
        Info::new(
            "chain_link_border",
            "Chain link border",
            EffectCategory::Filters,
            "image-effect/fence",
        )
        .desc(
            "Adds a metallic chain-link fence border with interlocking wire loops and highlights.",
        )
    }

    fn params() -> Vec<ParamDef> {
        vec![
            int_numeric::<Self>(
                "border_size",
                "Border size",
                16,
                250,
                52,
                |e: &mut Self, v| e.border_size = v,
            ),
            color_param::<Self>(
                "wire_color",
                "Wire color",
                SkColor::argb(0xFFAAAFB4),
                |e: &mut Self, v| e.wire_color = v,
            ),
            color_param::<Self>(
                "bg_color",
                "Background color",
                SkColor::argb(0xFF232830),
                |e: &mut Self, v| e.bg_color = v,
            ),
            float_slider::<Self>(
                "cell_size",
                "Cell size",
                8.0,
                60.0,
                20.0,
                |e: &mut Self, v| e.cell_size = v,
            ),
            float_slider::<Self>(
                "wire_width",
                "Wire width",
                1.0,
                8.0,
                2.5,
                |e: &mut Self, v| e.wire_width = v,
            ),
            bool_param::<Self>("highlight", "Wire highlight", true, |e: &mut Self, v| {
                e.highlight = v
            }),
        ]
    }

    fn apply(&self, source: &Bitmap) -> EffectResult {
        let border = self.border_size.clamp(16, 250);
        let cell = clampf(self.cell_size, 8.0, 60.0);
        let wire_w = clampf(self.wire_width, 1.0, 8.0);
        let (iw, ih) = (source.width() as i32, source.height() as i32);
        let (nw, nh) = (iw + border * 2, ih + border * 2);
        let mut out = Bitmap::new(nw.max(0) as u32, nh.max(0) as u32);
        let mut canvas = Canvas::new(&mut out);
        canvas.clear(self.bg_color.rgba_array());
        let half = cell * 0.5;
        let wc = self.wire_color;
        let wire = solid(wc, true);
        let wire_pen = Pen::new(wire_w).cap(Cap::Round);
        let lighten = |v: u8| clamp_to_byte(v as f32 + (255 - v) as f32 * 0.4);
        let hl = solid(
            SkColor::rgba(lighten(wc.r), lighten(wc.g), lighten(wc.b), wc.a),
            true,
        );
        let hl_pen = Pen::new(wire_w * 0.5).cap(Cap::Round);
        let hl = self.highlight.then_some((&hl, &hl_pen));
        let (bf, nwf) = (border as f32, nw as f32);
        mesh(
            &mut canvas,
            &wire,
            &wire_pen,
            hl,
            cell,
            half,
            0.0,
            0.0,
            nwf,
            bf,
        );
        mesh(
            &mut canvas,
            &wire,
            &wire_pen,
            hl,
            cell,
            half,
            0.0,
            (border + ih) as f32,
            nwf,
            bf,
        );
        mesh(
            &mut canvas,
            &wire,
            &wire_pen,
            hl,
            cell,
            half,
            0.0,
            bf,
            bf,
            ih as f32,
        );
        mesh(
            &mut canvas,
            &wire,
            &wire_pen,
            hl,
            cell,
            half,
            (border + iw) as f32,
            bf,
            bf,
            ih as f32,
        );
        let darken = |v: u8| clamp_to_byte(v as f32 * (1.0 - 0.3));
        let outline = solid(
            SkColor::rgba(darken(wc.r), darken(wc.g), darken(wc.b), wc.a),
            true,
        );
        sk::draw_rect_xywh(
            &mut canvas,
            (border - 1) as f32,
            (border - 1) as f32,
            (iw + 2) as f32,
            (ih + 2) as f32,
            &outline,
            Some(&Pen::new(wire_w * 1.5)),
        );
        canvas.draw_bitmap(source, bf, bf, None);
        drop(canvas);
        Ok(out)
    }
}

// ---- port ----

#[allow(clippy::too_many_arguments)]
fn mesh(
    canvas: &mut Canvas,
    wire: &Paint,
    wire_pen: &Pen,
    highlight: Option<(&Paint, &Pen)>,
    cell: f32,
    half: f32,
    bx: f32,
    by: f32,
    bw: f32,
    bh: f32,
) {
    sk::with_clip_rect(canvas, RRect::ltrb(bx, by, bx + bw, by + bh), |canvas| {
        let start_x = bx - cell * 2.0;
        let start_y = by - cell * 2.0;
        let end_x = bx + bw + cell * 2.0;
        let end_y = by + bh + cell * 2.0;
        let mut y = start_y;
        while y <= end_y {
            let mut x = start_x;
            while x <= end_x {
                let mut path = SkPath::new();
                path.move_to(x, y - half);
                path.line_to(x + half, y);
                path.line_to(x, y + half);
                path.line_to(x - half, y);
                path.line_to(x, y - half);
                sk::stroke_path(canvas, &path, wire_pen, wire);
                if let Some((p, pen)) = highlight {
                    let mut hl = SkPath::new();
                    hl.move_to(x - half, y);
                    hl.line_to(x, y - half);
                    hl.line_to(x + half, y);
                    sk::stroke_path(canvas, &hl, pen, p);
                }
                x += cell;
            }
            y += cell;
        }
    });
}
