//! An image of any size as egui textures: one texture when it fits the
//! GPU's largest texture side (egui's `max_texture_side`, which
//! [`crate::gpu::configure_context`] sets from the device), else a grid of
//! tiles drawn as one image.
//!
//! Large monitors, scrolling captures and large files can be wider or
//! taller than a weak or virtual GPU's limit (2048 on some VMs, remote
//! sessions and GL drivers): a texture over it is a wgpu validation error
//! (a panic). [`TiledTexture`] keeps every texture within the limit.
//!
//! Seams: each interior tile edge carries a one-pixel apron copied from
//! its neighbour, outside the part the tile draws, so bilinear filtering
//! at a seam reads the same two pixels one texture would, at any zoom;
//! nearest filtering never reads it. Adjacent tiles meet at the same
//! computed coordinate, so the rasteriser covers every pixel once. Mipmaps
//! would need a wider apron; no ShareXrs texture has them.

use egui::{Color32, ColorImage, Context, Painter, Rect, TextureHandle, TextureOptions, Vec2, pos2, vec2};

/// The pixels copied past an interior tile edge from the neighbouring tile.
pub const APRON: usize = 1;

/// One tile along one axis: the image pixels it draws (`start`, `len`) and
/// the pixels its texture holds (`tex_start`, `tex_len`: the drawn part and
/// its aprons).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Span {
    pub start: usize,
    pub len: usize,
    pub tex_start: usize,
    pub tex_len: usize,
}

/// Splits `len` pixels into tiles whose textures are at most `max` long:
/// one span when it fits, else spans of `max` texels (the last one shorter)
/// with an [`APRON`] on each interior side.
pub fn spans(len: usize, max: usize) -> Vec<Span> {
    assert!(max > 2 * APRON, "a texture side of {max} can't hold a tile and its aprons");
    if len <= max {
        return vec![Span { start: 0, len, tex_start: 0, tex_len: len }];
    }
    let mut out = Vec::new();
    let mut start = 0;
    while start < len {
        let tex_start = start.saturating_sub(APRON);
        let lead = start - tex_start;
        let rest = len - start;
        let len_here = if lead + rest <= max { rest } else { max - lead - APRON };
        let trail = if start + len_here < len { APRON } else { 0 };
        out.push(Span { start, len: len_here, tex_start, tex_len: lead + len_here + trail });
        start += len_here;
    }
    out
}

/// The largest texture side `ctx`'s renderer takes.
pub fn max_side(ctx: &Context) -> usize {
    ctx.input(|i| i.max_texture_side)
}

/// Whether an image of `size` needs more than one texture in `ctx`.
pub fn needs_tiles(ctx: &Context, size: [usize; 2]) -> bool {
    let max = max_side(ctx);
    size[0] > max || size[1] > max
}

#[derive(Clone)]
struct Tile {
    x: Span,
    y: Span,
    texture: TextureHandle,
}

/// An image uploaded as one texture or, past the GPU's limit, as tiles; see
/// the module documentation. Cloning shares the textures.
#[derive(Clone)]
pub struct TiledTexture {
    size: [usize; 2],
    options: TextureOptions,
    tiles: Vec<Tile>,
}

impl TiledTexture {
    /// Uploads `image` (moved into one texture when it fits).
    pub fn new(ctx: &Context, name: &str, image: ColorImage, options: TextureOptions) -> Self {
        Self::with_max(ctx, name, image, options, max_side(ctx))
    }

    fn with_max(ctx: &Context, name: &str, image: ColorImage, options: TextureOptions, max: usize) -> Self {
        let size = image.size;
        let (xs, ys) = (spans(size[0], max), spans(size[1], max));
        let tiles = if xs.len() == 1 && ys.len() == 1 {
            vec![Tile { x: xs[0], y: ys[0], texture: ctx.load_texture(name, image, options) }]
        } else {
            let mut tiles = Vec::with_capacity(xs.len() * ys.len());
            for y in &ys {
                for x in &xs {
                    let part = crop(&image, *x, *y);
                    let texture = ctx.load_texture(format!("{name} [{},{}]", x.start, y.start), part, options);
                    tiles.push(Tile { x: *x, y: *y, texture });
                }
            }
            tiles
        };
        Self { size, options, tiles }
    }

    /// Replaces the picture, as `TextureHandle::set`: into the same
    /// textures when the size is the same, else new ones.
    pub fn set(&mut self, ctx: &Context, name: &str, image: ColorImage, options: TextureOptions) {
        if image.size != self.size {
            *self = Self::new(ctx, name, image, options);
            return;
        }
        self.options = options;
        if let [tile] = self.tiles.as_mut_slice() {
            tile.texture.set(image, options);
            return;
        }
        for tile in &mut self.tiles {
            tile.texture.set(crop(&image, tile.x, tile.y), options);
        }
    }

    /// Writes a picture of the same size into the textures it has
    /// (`TextureHandle::set_partial` at the origin: the GPU textures are
    /// kept, not made again).
    pub fn write(&mut self, image: ColorImage) {
        debug_assert_eq!(image.size, self.size, "write needs a picture of the texture's size");
        let options = self.options;
        if let [tile] = self.tiles.as_mut_slice() {
            tile.texture.set_partial([0, 0], image, options);
            return;
        }
        for tile in &mut self.tiles {
            tile.texture.set_partial([0, 0], crop(&image, tile.x, tile.y), options);
        }
    }

    /// The image's size in pixels.
    pub fn size(&self) -> [usize; 2] {
        self.size
    }

    /// The image's size in pixels, as a vector.
    pub fn size_vec2(&self) -> Vec2 {
        vec2(self.size[0] as f32, self.size[1] as f32)
    }

    /// The filtering it was uploaded with.
    pub fn options(&self) -> TextureOptions {
        self.options
    }

    /// How many textures hold it (1 when it fits).
    pub fn texture_count(&self) -> usize {
        self.tiles.len()
    }

    /// The one texture, when it fits in one.
    pub fn single(&self) -> Option<&TextureHandle> {
        match self.tiles.as_slice() {
            [tile] => Some(&tile.texture),
            _ => None,
        }
    }

    /// Draws the whole image into `rect`, as `painter.image` with the full
    /// UV rectangle.
    pub fn paint(&self, painter: &Painter, rect: Rect, tint: Color32) {
        self.paint_uv(painter, rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)), tint);
    }

    /// Draws the part `uv` (0 to 1 of the whole image) into `rect`, as
    /// `painter.image(id, rect, uv, tint)` would with one texture.
    pub fn paint_uv(&self, painter: &Painter, rect: Rect, uv: Rect, tint: Color32) {
        if let Some(texture) = self.single() {
            painter.image(texture.id(), rect, uv, tint);
            return;
        }
        for (r, tile_uv, id) in self.parts(rect, uv) {
            painter.image(id, r, tile_uv, tint);
        }
    }

    /// Each tile's screen rectangle and UV rectangle for drawing `uv` of
    /// the image into `rect`.
    fn parts(&self, rect: Rect, uv: Rect) -> Vec<(Rect, Rect, egui::TextureId)> {
        let (w, h) = (self.size[0] as f32, self.size[1] as f32);
        // The drawn part in image pixels, and screen units per pixel.
        let (u0, u1, v0, v1) = (uv.min.x * w, uv.max.x * w, uv.min.y * h, uv.max.y * h);
        if u1 == u0 || v1 == v0 {
            return Vec::new();
        }
        let (sx, sy) = (rect.width() / (u1 - u0), rect.height() / (v1 - v0));
        let mut out = Vec::new();
        for tile in &self.tiles {
            let Some((a, b)) = overlap(tile.x, u0, u1) else { continue };
            let Some((c, d)) = overlap(tile.y, v0, v1) else { continue };
            let screen = Rect::from_min_max(
                pos2(rect.min.x + (a - u0) * sx, rect.min.y + (c - v0) * sy),
                pos2(rect.min.x + (b - u0) * sx, rect.min.y + (d - v0) * sy),
            );
            let (tx, tw) = (tile.x.tex_start as f32, tile.x.tex_len as f32);
            let (ty, th) = (tile.y.tex_start as f32, tile.y.tex_len as f32);
            let tile_uv = Rect::from_min_max(pos2((a - tx) / tw, (c - ty) / th), pos2((b - tx) / tw, (d - ty) / th));
            out.push((screen, tile_uv, tile.texture.id()));
        }
        out
    }
}

/// The part of `[lo, hi]` (image pixels, either order) the span draws.
fn overlap(span: Span, lo: f32, hi: f32) -> Option<(f32, f32)> {
    let (s, e) = (span.start as f32, (span.start + span.len) as f32);
    let flip = hi < lo;
    let (lo, hi) = if flip { (hi, lo) } else { (lo, hi) };
    let (a, b) = (s.max(lo), e.min(hi));
    if a >= b {
        return None;
    }
    Some(if flip { (b, a) } else { (a, b) })
}

/// The texels of tile `x`, `y` (aprons included).
fn crop(image: &ColorImage, x: Span, y: Span) -> ColorImage {
    let stride = image.size[0];
    let mut pixels = Vec::with_capacity(x.tex_len * y.tex_len);
    for row in y.tex_start..y.tex_start + y.tex_len {
        let from = row * stride + x.tex_start;
        pixels.extend_from_slice(&image.pixels[from..from + x.tex_len]);
    }
    ColorImage::new([x.tex_len, y.tex_len], pixels)
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Every pixel is drawn by exactly one span, each texture fits, and the
    /// aprons are exactly one pixel on interior sides only.
    fn check(len: usize, max: usize) {
        let s = spans(len, max);
        let mut next = 0;
        for (i, sp) in s.iter().enumerate() {
            assert_eq!(sp.start, next, "{len}/{max}: contiguous");
            assert!(sp.len > 0 || len == 0);
            assert!(sp.tex_len <= max, "{len}/{max}: {sp:?} fits");
            let lead = if i == 0 { 0 } else { APRON };
            let trail = if i + 1 == s.len() { 0 } else { APRON };
            assert_eq!(sp.tex_start + lead, sp.start, "{len}/{max}: leading apron");
            assert_eq!(sp.tex_len, lead + sp.len + trail, "{len}/{max}: trailing apron");
            next += sp.len;
        }
        assert_eq!(next, len);
        if len <= max {
            assert_eq!(s.len(), 1);
        } else {
            // As few tiles as the aprons allow: all but the last are full.
            assert!(s[..s.len() - 1].iter().all(|sp| sp.tex_len == max));
        }
    }

    #[test]
    fn a_side_that_fits_is_one_span() {
        assert_eq!(spans(2048, 2048), vec![Span { start: 0, len: 2048, tex_start: 0, tex_len: 2048 }]);
        assert_eq!(spans(1, 2048), vec![Span { start: 0, len: 1, tex_start: 0, tex_len: 1 }]);
        assert_eq!(spans(0, 2048), vec![Span { start: 0, len: 0, tex_start: 0, tex_len: 0 }]);
    }

    #[test]
    fn a_2560_screen_on_2048_is_two_tiles_with_aprons() {
        assert_eq!(
            spans(2560, 2048),
            vec![
                Span { start: 0, len: 2047, tex_start: 0, tex_len: 2048 },
                Span { start: 2047, len: 513, tex_start: 2046, tex_len: 514 },
            ]
        );
    }

    #[test]
    fn edge_tiles_and_odd_sizes() {
        // One past a tile: the last tile draws one pixel.
        assert_eq!(spans(2049, 2048).last(), Some(&Span { start: 2047, len: 2, tex_start: 2046, tex_len: 3 }));
        // Three tiles: the middle one has aprons on both sides.
        let s = spans(5000, 2048);
        assert_eq!(s.len(), 3);
        assert_eq!(s[1], Span { start: 2047, len: 2046, tex_start: 2046, tex_len: 2048 });
        for len in [0, 1, 2, 3, 7, 63, 64, 65, 127, 1023, 1024, 1025, 2047, 2048, 2049, 2560, 4093, 4094, 4095, 4096, 4097, 10_001, 65_537] {
            for max in [3, 4, 5, 64, 1000, 1024, 2048, 4096] {
                check(len, max);
            }
        }
    }

    fn pattern(w: usize, h: usize) -> ColorImage {
        let pixels = (0..w * h).map(|i| Color32::from_rgba_premultiplied((i % 251) as u8, (i / w % 253) as u8, (i * 7 % 256) as u8, 255)).collect();
        ColorImage::new([w, h], pixels)
    }

    #[test]
    fn tiles_hold_the_image_and_their_aprons() {
        let ctx = Context::default();
        let image = pattern(301, 157);
        let t = TiledTexture::with_max(&ctx, "t", image.clone(), TextureOptions::LINEAR, 64);
        assert_eq!(t.size(), [301, 157]);
        assert_eq!(t.texture_count(), spans(301, 64).len() * spans(157, 64).len());
        assert!(t.single().is_none());
        for tile in &t.tiles {
            let want = crop(&image, tile.x, tile.y);
            assert_eq!(tile.texture.size(), want.size);
            assert!(tile.texture.size()[0] <= 64 && tile.texture.size()[1] <= 64);
        }
        let one = TiledTexture::with_max(&ctx, "one", image, TextureOptions::LINEAR, 2048);
        assert_eq!(one.texture_count(), 1);
        assert_eq!(one.single().map(|t| t.size()), Some([301, 157]));
    }

    #[test]
    fn the_crop_is_the_tile_and_its_aprons() {
        let image = pattern(10, 4);
        let part = crop(&image, Span { start: 4, len: 3, tex_start: 3, tex_len: 5 }, Span { start: 0, len: 4, tex_start: 0, tex_len: 4 });
        assert_eq!(part.size, [5, 4]);
        for y in 0..4 {
            for x in 0..5 {
                assert_eq!(part.pixels[y * 5 + x], image.pixels[y * 10 + x + 3]);
            }
        }
    }

    #[test]
    fn parts_cover_the_rect_and_meet_at_the_seams() {
        let ctx = Context::default();
        let t = TiledTexture::with_max(&ctx, "t", pattern(300, 200), TextureOptions::LINEAR, 128);
        let rect = Rect::from_min_size(pos2(10.0, 20.0), vec2(375.0, 250.0));
        let parts = t.parts(rect, Rect::from_min_max(pos2(0.0, 0.0), pos2(1.0, 1.0)));
        assert_eq!(parts.len(), t.texture_count());
        let area: f32 = parts.iter().map(|(r, _, _)| r.area()).sum();
        assert!((area - rect.area()).abs() < 0.5, "{area} vs {}", rect.area());
        let xs: Vec<f32> = parts.iter().flat_map(|(r, _, _)| [r.min.x, r.max.x]).collect();
        // Every inner edge is shared: 300 px at 1.25, tiles drawing 127, 126, 47.
        for edge in [10.0 + 127.0 * 1.25, 10.0 + 253.0 * 1.25] {
            assert_eq!(xs.iter().filter(|x| **x == edge).count(), 2 * 2, "edge {edge}");
        }
        // The UVs land on the drawn texels, inside the aprons.
        let (r, uv, _) = parts[1];
        assert_eq!(r.min.x, 10.0 + 127.0 * 1.25);
        assert_eq!(uv.min.x, 1.0 / 128.0);
        assert_eq!(uv.max.x, 127.0 / 128.0);
    }

    #[test]
    fn a_partial_uv_draws_only_the_tiles_it_covers() {
        let ctx = Context::default();
        let t = TiledTexture::with_max(&ctx, "t", pattern(300, 200), TextureOptions::NEAREST, 128);
        // Pixels 10..100 x 0..50: inside the first tile.
        let uv = Rect::from_min_max(pos2(10.0 / 300.0, 0.0), pos2(100.0 / 300.0, 50.0 / 200.0));
        let parts = t.parts(Rect::from_min_size(pos2(0.0, 0.0), vec2(90.0, 50.0)), uv);
        assert_eq!(parts.len(), 1);
        let (r, tuv, _) = parts[0];
        assert!((r.max.x - 90.0).abs() < 1e-3 && (r.max.y - 50.0).abs() < 1e-3);
        assert!((tuv.min.x - 10.0 / 128.0).abs() < 1e-6 && (tuv.max.x - 100.0 / 128.0).abs() < 1e-6);
    }

    /// Drawn by the GPU, tiles look like the one texture: identical when
    /// texels land on whole device pixels (the overlay's frozen screen at
    /// any scaling, an image at 100%), and otherwise apart only where the
    /// float rounding of the UVs differs: a few levels of a filtered
    /// texel's weights, or which texel a sample exactly on a texel edge
    /// takes (nearest filtering at 150%, a tie either way). No seams.
    #[test]
    fn tiles_draw_as_one_texture_on_the_gpu() {
        let (w, h) = (301, 203);
        // Noise, so a wrong texel or weight anywhere shows.
        let mut seed = 0x2545_f491_u32;
        let pixels = (0..w * h)
            .map(|_| {
                seed ^= seed << 13;
                seed ^= seed >> 17;
                seed ^= seed << 5;
                let [r, g, b, a] = seed.to_le_bytes();
                Color32::from_rgba_unmultiplied(r, g, b, a | 0x80)
            })
            .collect();
        let image = ColorImage::new([w, h], pixels);
        // (pixels per point, image pixels to device pixels, where, aligned)
        let cases: [(f32, f32, Vec2, bool); 8] = [
            (1.0, 1.0, vec2(3.0, 5.0), true),
            (1.25, 1.0, vec2(3.2, 4.8), true),
            (1.0, 3.0, vec2(-101.0, -50.0), true),
            (1.25, 1.0, vec2(2.5, 7.7), false),
            (1.0, 0.37, vec2(10.3, 4.1), false),
            (1.0, 2.6, vec2(-40.2, -13.7), false),
            (1.0, 1.5, vec2(3.0, 5.0), false),
            (1.25, 1.5, vec2(3.2, 4.8), false),
        ];
        for (ppp, zoom, at, aligned) in cases {
            let mut screen = crate::offscreen::Offscreen::new([400, 300], ppp, true).expect("a GPU");
            for options in [TextureOptions::NEAREST, TextureOptions::LINEAR] {
                let mut shots = Vec::new();
                for max in [4096, 64] {
                    let mut tex: Option<TiledTexture> = None;
                    shots.push(screen.render(|ui| {
                        let t = tex.get_or_insert_with(|| TiledTexture::with_max(ui.ctx(), "t", image.clone(), options, max));
                        let rect = Rect::from_min_size(at.to_pos2(), vec2(w as f32, h as f32) * zoom / ppp);
                        t.paint(ui.painter(), rect, Color32::WHITE);
                    }));
                    assert_eq!(tex.map(|t| t.texture_count() > 1), Some(max == 64));
                }
                let (one, tiled) = (&shots[0], &shots[1]);
                let what = format!("{:?} at {ppp}x, zoom {zoom}, {at:?}", options.magnification);
                // Magnified linear filtering interpolates in the GPU's own
                // precision: a hardware GPU gives the same weights to a tile
                // and to the one texture, but a software rasteriser (WARP, on
                // CI's Windows runners) may round a texel's weights a level
                // apart where a tile's UVs differ. Hold it to the same few
                // levels as unaligned draws there.
                let exact = aligned && !(options == TextureOptions::LINEAR && zoom > 1.0 && screen.is_software());
                if exact {
                    assert!(one == tiled, "{what}: not identical");
                } else if options == TextureOptions::LINEAR {
                    let worst = one.as_raw().iter().zip(tiled.as_raw()).map(|(a, b)| a.abs_diff(*b)).max().unwrap_or(0);
                    assert!(worst <= 3, "{what}: {worst} levels apart");
                } else if zoom >= 1.0 {
                    // A tie taken the other way shows the neighbouring texel,
                    // which the one texture shows in a neighbouring pixel
                    // (dithered there: a level or two apart).
                    for (x, y, p) in tiled.enumerate_pixels() {
                        let near = (-1..=1).flat_map(|dy| (-1..=1).map(move |dx| (x as i64 + dx, y as i64 + dy)));
                        let found = near.filter(|&(nx, ny)| nx >= 0 && ny >= 0 && (nx as u32) < one.width() && (ny as u32) < one.height()).any(|(nx, ny)| one.get_pixel(nx as u32, ny as u32).0.iter().zip(p.0).all(|(a, b)| a.abs_diff(b) <= 2));
                        assert!(found, "{what}: pixel {x},{y} isn't one of the texture's");
                    }
                }
            }
        }
    }
}
