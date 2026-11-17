//! ShareX.HelpersLib's GIF writer, ported byte for byte: `IndexedImage`
//! (header, frames, LZW), `AnimatedGifCreator`, and the quantizers
//! (`AdaptiveGifQuantizer` with `GifPaletteMapper`'s Sierra Lite dithering,
//! `SkiaOctreeQuantizer`, `SkiaGrayscaleQuantizer`).

use std::collections::{HashMap, HashSet};

use crate::dotnet::{PriorityQueue, cmp_f64, intro_sort};
use crate::imaging::{Bitmap, Rgba, straight_pixels};

/// `IndexedImage`: palette pixels for the GIF encoder. Colours are straight RGBA.
#[derive(Debug, Clone, PartialEq)]
pub struct IndexedImage {
    pub width: u32,
    pub height: u32,
    pub pixels: Vec<u8>,
    pub palette: Vec<Rgba>,
}

impl IndexedImage {
    pub fn new(width: u32, height: u32, pixels: Vec<u8>, palette: Vec<Rgba>) -> Self {
        assert!(width >= 1 && height >= 1, "IndexedImage size");
        assert!(pixels.len() == (width * height) as usize && (1..=256).contains(&palette.len()));
        IndexedImage { width, height, pixels, palette }
    }

    /// `SaveGif`.
    pub fn save_gif(&self) -> Vec<u8> {
        let mut out = Vec::new();
        self.write_header(&mut out, false);
        self.write_frame(&mut out, 0, 0, 0, true, false, None);
        out.push(0x3B);
        out
    }

    fn palette_bits(&self) -> u32 {
        let mut bits = 1;
        while (1usize << bits) < self.palette.len() {
            bits += 1;
        }
        bits
    }

    fn write_palette(&self, out: &mut Vec<u8>, bits: u32) {
        for i in 0..(1usize << bits) {
            let c = self.palette.get(i).copied().unwrap_or([0, 0, 0, 255]);
            out.extend_from_slice(&[c[0], c[1], c[2]]);
        }
    }

    pub(crate) fn write_header(&self, out: &mut Vec<u8>, write_palette: bool) {
        out.extend_from_slice(b"GIF89a");
        out.extend_from_slice(&(self.width as u16).to_le_bytes());
        out.extend_from_slice(&(self.height as u16).to_le_bytes());
        let bits = self.palette_bits();
        out.push((0x70 | if write_palette { 0x80 | (bits - 1) } else { 0 }) as u8);
        out.push(0);
        out.push(0);
        if write_palette {
            self.write_palette(out, bits);
        }
    }

    #[allow(clippy::too_many_arguments)]
    pub(crate) fn write_frame(
        &self,
        out: &mut Vec<u8>,
        delay: i32,
        left: u16,
        top: u16,
        restore_background: bool,
        use_global_palette: bool,
        transparent_index: Option<i32>,
    ) {
        let transparent =
            transparent_index.unwrap_or_else(|| self.palette.iter().position(|c| c[3] < 128).map_or(-1, |i| i as i32));
        let bits = self.palette_bits();
        out.extend_from_slice(&[0x21, 0xF9, 4]);
        out.push(((if restore_background { 8 } else { 4 }) | if transparent >= 0 { 1 } else { 0 }) as u8);
        out.extend_from_slice(&((delay / 10).clamp(0, u16::MAX as i32) as u16).to_le_bytes());
        out.push(transparent.max(0) as u8);
        out.push(0);
        out.push(0x2C);
        out.extend_from_slice(&left.to_le_bytes());
        out.extend_from_slice(&top.to_le_bytes());
        out.extend_from_slice(&(self.width as u16).to_le_bytes());
        out.extend_from_slice(&(self.height as u16).to_le_bytes());
        out.push(if use_global_palette { 0 } else { (0x80 | (bits - 1)) as u8 });
        if !use_global_palette {
            self.write_palette(out, bits);
        }
        let min_code_size = bits.max(2);
        out.push(min_code_size as u8);
        let data = compress(&self.pixels, min_code_size);
        for chunk in data.chunks(255) {
            out.push(chunk.len() as u8);
            out.extend_from_slice(chunk);
        }
        out.push(0);
    }
}

/// `IndexedImage.Compress`: GIF LZW, little-endian bit packing, 12-bit codes.
fn compress(pixels: &[u8], min_code_size: u32) -> Vec<u8> {
    let mut out = Vec::new();
    let mut dict = vec![0i32; 1 << 20];
    let clear = 1i32 << min_code_size;
    let end = clear + 1;
    let mut next = end + 1;
    let mut size = min_code_size + 1;
    let mut generation = 1i32 << 12;
    let mut buffer: u32 = 0;
    let mut buffered = 0u32;
    let write = |code: i32, size: u32, buffer: &mut u32, buffered: &mut u32, out: &mut Vec<u8>| {
        *buffer |= (code as u32) << *buffered;
        *buffered += size;
        while *buffered >= 8 {
            out.push(*buffer as u8);
            *buffer >>= 8;
            *buffered -= 8;
        }
    };
    write(clear, size, &mut buffer, &mut buffered, &mut out);
    let mut prefix = pixels[0] as i32;
    for &pixel in &pixels[1..] {
        let key = ((prefix << 8) | pixel as i32) as usize;
        let entry = dict[key];
        if (entry & !4095) == generation {
            prefix = entry & 4095;
            continue;
        }
        write(prefix, size, &mut buffer, &mut buffered, &mut out);
        if next < 4096 {
            dict[key] = generation | next;
            next += 1;
            if next > (1 << size) && size < 12 {
                size += 1;
            }
        } else {
            write(clear, size, &mut buffer, &mut buffered, &mut out);
            generation += 1 << 12;
            next = end + 1;
            size = min_code_size + 1;
        }
        prefix = pixel as i32;
    }
    write(prefix, size, &mut buffer, &mut buffered, &mut out);
    if next == (1 << size) && size < 12 {
        size += 1;
    }
    write(end, size, &mut buffer, &mut buffered, &mut out);
    if buffered > 0 {
        out.push(buffer as u8);
    }
    out
}

/// `AnimatedGifCreator` writing into memory.
pub struct AnimatedGifCreator {
    pub delay: i32,
    pub repeat: i32,
    pub looping: bool,
    pub use_global_palette: bool,
    pub frame_count: usize,
    out: Vec<u8>,
    started: bool,
    global_palette: Option<Vec<Rgba>>,
}

impl AnimatedGifCreator {
    pub fn new(delay: i32, repeat: i32, looping: bool) -> Self {
        AnimatedGifCreator {
            delay,
            repeat,
            looping,
            use_global_palette: false,
            frame_count: 0,
            out: Vec::new(),
            started: false,
            global_palette: None,
        }
    }

    /// `AddFrame(bitmap, quality)`.
    pub fn add_bitmap(&mut self, bmp: &Bitmap, quality: crate::imaging::GifQuality) {
        let image = crate::imaging::quantize(bmp, quality);
        let delay = self.delay;
        self.add_frame(&image, delay, 0, 0, true, None);
    }

    pub fn add_frame(
        &mut self,
        image: &IndexedImage,
        delay: i32,
        left: u16,
        top: u16,
        restore_background: bool,
        transparent_index: Option<i32>,
    ) {
        if !self.started {
            self.started = true;
            self.global_palette = self.use_global_palette.then(|| image.palette.clone());
            image.write_header(&mut self.out, self.use_global_palette);
            if self.looping {
                self.out.extend_from_slice(&[0x21, 0xFF, 0x0B]);
                self.out.extend_from_slice(b"NETSCAPE2.0");
                self.out.extend_from_slice(&[
                    0x03,
                    0x01,
                    (self.repeat % 0x100) as u8,
                    (self.repeat / 0x100) as u8,
                    0x00,
                ]);
            }
        }
        let matches = self.global_palette.as_ref().is_some_and(|g| {
            g.len() == image.palette.len() && g.iter().zip(&image.palette).all(|(a, b)| a[..3] == b[..3])
        });
        image.write_frame(&mut self.out, delay, left, top, restore_background, matches, transparent_index);
        self.frame_count += 1;
    }

    /// `Dispose`: the trailer, and the file's bytes.
    pub fn finish(mut self) -> Vec<u8> {
        if self.started {
            self.out.push(0x3B);
        }
        self.out
    }
}

// ------------------------------------------------------------ adaptive

const COLOR_BITS: i32 = 6;
const SIDE: i32 = 1 << COLOR_BITS;
const HSIDE: i32 = SIDE + 1;
const HLEN: usize = (HSIDE * HSIDE * HSIDE) as usize;

fn hindex(r: i32, g: i32, b: i32) -> usize {
    ((r * HSIDE + g) * HSIDE + b) as usize
}

#[derive(Clone, Copy, Default)]
struct Cube {
    r0: i32,
    r1: i32,
    g0: i32,
    g1: i32,
    b0: i32,
    b1: i32,
}

#[derive(Clone, Copy, PartialEq)]
enum Axis {
    None,
    Red,
    Green,
    Blue,
}

/// A `ColorFrequency` shared between `_flatColors` and the priority queue.
type Freq = std::rc::Rc<std::cell::Cell<(i32, i64, i64)>>;

struct Moments {
    weights: Vec<i64>,
    reds: Vec<i64>,
    greens: Vec<i64>,
    blues: Vec<i64>,
    exact: Option<HashSet<i32>>,
    flat: HashMap<i32, Freq>,
    candidates: PriorityQueue<Freq, i64>,
    opaque_pixels: i64,
    distinct: i32,
    has_transparency: bool,
}

const MIN_FLAT_RUN: i32 = 8;
const MAX_FLAT_CANDIDATES: usize = 64;
const MAX_PRESERVED: usize = 32;

impl Moments {
    fn new() -> Self {
        Moments {
            weights: vec![0; HLEN],
            reds: vec![0; HLEN],
            greens: vec![0; HLEN],
            blues: vec![0; HLEN],
            exact: Some(HashSet::new()),
            flat: HashMap::new(),
            candidates: PriorityQueue::new(|a: &i64, b: &i64| a.cmp(b)),
            opaque_pixels: 0,
            distinct: 0,
            has_transparency: false,
        }
    }

    fn add_pixels(&mut self, px: &[Rgba], width: usize, height: usize) {
        let mut previous = -1i32;
        let mut opaque = 0i64;
        for y in 0..height {
            let (mut run_color, mut run_len) = (-1i32, 0i32);
            for x in 0..width {
                let p = px[y * width + x];
                if p[3] < 128 {
                    self.has_transparency = true;
                    if run_len >= MIN_FLAT_RUN {
                        self.add_flat_run(run_color, run_len);
                    }
                    run_color = -1;
                    run_len = 0;
                    continue;
                }
                let (r, g, b) = (p[0] as i32, p[1] as i32, p[2] as i32);
                let color = (r << 16) | (g << 8) | b;
                opaque += 1;
                if color == run_color {
                    run_len += 1;
                } else {
                    if run_len >= MIN_FLAT_RUN {
                        self.add_flat_run(run_color, run_len);
                    }
                    run_color = color;
                    run_len = 1;
                }
                if color != previous {
                    if let Some(set) = &mut self.exact {
                        set.insert(color);
                        if set.len() > 256 {
                            self.exact = None;
                        }
                        previous = color;
                    }
                }
                let i = hindex((r >> 2) + 1, (g >> 2) + 1, (b >> 2) + 1);
                if self.weights[i] == 0 {
                    self.distinct += 1;
                }
                self.weights[i] += 1;
                self.reds[i] += r as i64;
                self.greens[i] += g as i64;
                self.blues[i] += b as i64;
            }
            if run_len >= MIN_FLAT_RUN {
                self.add_flat_run(run_color, run_len);
            }
        }
        self.opaque_pixels += opaque;
    }

    fn add_flat_run(&mut self, color: i32, length: i32) {
        if let Some(f) = self.flat.get(&color) {
            let (c, n, e) = f.get();
            f.set((c, n + length as i64, e));
            return;
        }
        let freq: Freq = if self.flat.len() < MAX_FLAT_CANDIDATES {
            std::rc::Rc::new(std::cell::Cell::new((color, length as i64, 0)))
        } else {
            while let Some((cand, count)) = self.candidates.peek().cloned() {
                if cand.get().1 == count {
                    break;
                }
                self.candidates.dequeue();
                let n = cand.get().1;
                self.candidates.enqueue(cand, n);
            }
            let (f, _) = self.candidates.dequeue().expect("candidate");
            let (old_color, minimum, _) = f.get();
            self.flat.remove(&old_color);
            f.set((color, minimum + length as i64, minimum));
            f
        };
        let n = freq.get().1;
        self.flat.insert(color, freq.clone());
        self.candidates.enqueue(freq, n);
    }

    fn volume(c: &Cube, m: &[i64]) -> i64 {
        m[hindex(c.r1, c.g1, c.b1)] - m[hindex(c.r1, c.g1, c.b0)] - m[hindex(c.r1, c.g0, c.b1)]
            + m[hindex(c.r1, c.g0, c.b0)]
            - m[hindex(c.r0, c.g1, c.b1)]
            + m[hindex(c.r0, c.g1, c.b0)]
            + m[hindex(c.r0, c.g0, c.b1)]
            - m[hindex(c.r0, c.g0, c.b0)]
    }

    fn score(r: i64, g: i64, b: i64, w: i64) -> f64 {
        (2.0 * r as f64 * r as f64 + 4.0 * g as f64 * g as f64 + 3.0 * b as f64 * b as f64) / w as f64
    }

    fn create_palette(&mut self, max_colors: usize) -> Vec<Rgba> {
        if self.distinct == 0 {
            return vec![[0, 0, 0, 255]];
        }
        if let Some(set) = &self.exact {
            if set.len() <= max_colors {
                let mut v: Vec<i32> = set.iter().copied().collect();
                v.sort();
                return v.into_iter().map(rgb).collect();
            }
        }
        let threshold = (MIN_FLAT_RUN as i64).max(self.opaque_pixels / 256);
        let mut preserved: Vec<(i32, i64)> = self
            .flat
            .iter()
            .map(|(k, f)| (*k, f.get().1 - f.get().2))
            .filter(|(_, n)| *n >= threshold)
            .collect();
        preserved.sort_by(|a, b| b.1.cmp(&a.1).then(a.0.cmp(&b.0)));
        preserved.truncate(MAX_PRESERVED);
        for &(key, count) in &preserved {
            let (r, g, b) = ((key >> 16) & 255, (key >> 8) & 255, key & 255);
            let i = hindex((r >> 2) + 1, (g >> 2) + 1, (b >> 2) + 1);
            self.weights[i] -= count;
            self.reds[i] -= r as i64 * count;
            self.greens[i] -= g as i64 * count;
            self.blues[i] -= b as i64 * count;
            if self.weights[i] == 0 {
                self.distinct -= 1;
            }
        }
        let flat_palette: Vec<Rgba> = preserved.iter().map(|(k, _)| rgb(*k)).collect();
        if self.distinct == 0 {
            return flat_palette;
        }
        self.cumulative();
        let target = (max_colors - flat_palette.len()).min(self.distinct as usize);
        let mut cubes = vec![Cube { r1: SIDE, g1: SIDE, b1: SIDE, ..Default::default() }];
        type Split = (usize, Cube, Cube);
        let mut splits: PriorityQueue<Split, (f64, i32)> =
            PriorityQueue::new(|a: &(f64, i32), b: &(f64, i32)| cmp_f64(&a.0, &b.0).then(a.1.cmp(&b.1)));
        let queue = |this: &Moments, cubes: &Vec<Cube>, splits: &mut PriorityQueue<Split, (f64, i32)>, index: usize| {
            if let Some((first, second, gain)) = this.try_split(&cubes[index]) {
                splits.enqueue((index, first, second), (-gain, index as i32));
            }
        };
        queue(self, &cubes, &mut splits, 0);
        while cubes.len() < target {
            let Some(((index, first, second), _)) = splits.dequeue() else { break };
            cubes[index] = first;
            let second_index = cubes.len();
            cubes.push(second);
            queue(self, &cubes, &mut splits, index);
            queue(self, &cubes, &mut splits, second_index);
        }
        let mut palette = flat_palette;
        for cube in &cubes {
            let w = Self::volume(cube, &self.weights);
            let ch = |m: &[i64]| ((Self::volume(cube, m) + w / 2) / w).clamp(0, 255) as u8;
            palette.push([ch(&self.reds), ch(&self.greens), ch(&self.blues), 255]);
        }
        palette
    }

    fn cumulative(&mut self) {
        let n = HSIDE as usize;
        let (mut aw, mut ar, mut ag, mut ab) = (vec![0i64; n], vec![0i64; n], vec![0i64; n], vec![0i64; n]);
        for r in 1..=SIDE {
            aw.fill(0);
            ar.fill(0);
            ag.fill(0);
            ab.fill(0);
            for g in 1..=SIDE {
                let (mut lw, mut lr, mut lg, mut lb) = (0i64, 0i64, 0i64, 0i64);
                for b in 1..=SIDE {
                    let i = hindex(r, g, b);
                    lw += self.weights[i];
                    lr += self.reds[i];
                    lg += self.greens[i];
                    lb += self.blues[i];
                    let bu = b as usize;
                    aw[bu] += lw;
                    ar[bu] += lr;
                    ag[bu] += lg;
                    ab[bu] += lb;
                    let p = hindex(r - 1, g, b);
                    self.weights[i] = self.weights[p] + aw[bu];
                    self.reds[i] = self.reds[p] + ar[bu];
                    self.greens[i] = self.greens[p] + ag[bu];
                    self.blues[i] = self.blues[p] + ab[bu];
                }
            }
        }
    }

    fn try_split(&self, cube: &Cube) -> Option<(Cube, Cube, f64)> {
        let tw = Self::volume(cube, &self.weights);
        let tr = Self::volume(cube, &self.reds);
        let tg = Self::volume(cube, &self.greens);
        let tb = Self::volume(cube, &self.blues);
        let original = Self::score(tr, tg, tb, tw);
        let mut best = f64::NEG_INFINITY;
        let mut best_axis = Axis::None;
        let mut best_cut = 0;
        for (axis, lo, hi) in [
            (Axis::Red, cube.r0 + 1, cube.r1),
            (Axis::Green, cube.g0 + 1, cube.g1),
            (Axis::Blue, cube.b0 + 1, cube.b1),
        ] {
            for cut in lo..hi {
                let mut cand = *cube;
                match axis {
                    Axis::Red => cand.r1 = cut,
                    Axis::Green => cand.g1 = cut,
                    _ => cand.b1 = cut,
                }
                let fw = Self::volume(&cand, &self.weights);
                let sw = tw - fw;
                if fw == 0 || sw == 0 {
                    continue;
                }
                let fr = Self::volume(&cand, &self.reds);
                let fg = Self::volume(&cand, &self.greens);
                let fb = Self::volume(&cand, &self.blues);
                let s = Self::score(fr, fg, fb, fw) + Self::score(tr - fr, tg - fg, tb - fb, sw);
                if s > best {
                    best = s;
                    best_axis = axis;
                    best_cut = cut;
                }
            }
        }
        let (mut first, mut second) = (*cube, *cube);
        match best_axis {
            Axis::Red => {
                first.r1 = best_cut;
                second.r0 = best_cut;
            }
            Axis::Green => {
                first.g1 = best_cut;
                second.g0 = best_cut;
            }
            Axis::Blue => {
                first.b1 = best_cut;
                second.b0 = best_cut;
            }
            Axis::None => return None,
        }
        let gain = best - original;
        (gain > 0.0).then_some((first, second, gain))
    }
}

fn rgb(c: i32) -> Rgba {
    [(c >> 16) as u8, (c >> 8) as u8, c as u8, 255]
}

/// `AdaptiveGifQuantizer`.
pub struct AdaptiveGifQuantizer {
    moments: Option<Moments>,
    mapper: Option<PaletteMapper>,
    has_frames: bool,
}

impl Default for AdaptiveGifQuantizer {
    fn default() -> Self {
        Self::new()
    }
}

impl AdaptiveGifQuantizer {
    pub fn new() -> Self {
        AdaptiveGifQuantizer { moments: Some(Moments::new()), mapper: None, has_frames: false }
    }

    /// `AddFrame`: counts a frame's colours before the palette is built.
    pub fn add_frame(&mut self, bmp: &Bitmap) {
        assert!(self.mapper.is_none(), "Add frames before quantizing the palette.");
        let px = straight_pixels(bmp);
        self.moments.as_mut().unwrap().add_pixels(&px, bmp.width() as usize, bmp.height() as usize);
        self.has_frames = true;
    }

    pub fn quantize(&mut self, bmp: &Bitmap) -> IndexedImage {
        let px = straight_pixels(bmp);
        if self.mapper.is_none() {
            let mut m = self.moments.take().unwrap();
            if !self.has_frames {
                m.add_pixels(&px, bmp.width() as usize, bmp.height() as usize);
            }
            let colors = m.create_palette(if m.has_transparency { 255 } else { 256 });
            self.mapper = Some(PaletteMapper::new(&colors, m.has_transparency));
        }
        self.mapper.as_mut().unwrap().quantize(&px, bmp.width(), bmp.height())
    }
}

#[derive(Clone, Copy)]
struct Node {
    r: i32,
    g: i32,
    b: i32,
    index: u8,
    axis: i32,
    left: i32,
    right: i32,
}

/// `GifPaletteMapper`.
struct PaletteMapper {
    palette: Vec<Rgba>,
    rgb: Vec<i32>,
    exact: HashMap<i32, u8>,
    nodes: Vec<Node>,
    root: i32,
    transparent: i32,
    cached_colors: Vec<i32>,
    cached_indices: Vec<u8>,
}

impl PaletteMapper {
    fn new(colors: &[Rgba], transparency: bool) -> Self {
        let transparent = if transparency { colors.len() as i32 } else { -1 };
        let mut palette = colors.to_vec();
        let mut exact = HashMap::new();
        let rgbv: Vec<i32> =
            colors.iter().map(|c| ((c[0] as i32) << 16) | ((c[1] as i32) << 8) | c[2] as i32).collect();
        for (i, c) in rgbv.iter().enumerate() {
            exact.entry(*c).or_insert(i as u8);
        }
        if transparency {
            // Color.Transparent: ARGB 0, 255, 255, 255.
            palette.push([255, 255, 255, 0]);
        }
        let mut m = PaletteMapper {
            palette,
            rgb: rgbv,
            exact,
            nodes: vec![
                Node { r: 0, g: 0, b: 0, index: 0, axis: 0, left: -1, right: -1 };
                colors.len()
            ],
            root: -1,
            transparent,
            cached_colors: vec![-1; 1 << 20],
            cached_indices: vec![0; 1 << 20],
        };
        let mut order: Vec<u8> = (0..colors.len()).map(|i| i as u8).collect();
        let mut count = 0usize;
        m.root = m.build(&mut order, 0, colors.len(), &mut count);
        m
    }

    fn build(&mut self, order: &mut [u8], start: usize, count: usize, node_count: &mut usize) -> i32 {
        if count == 0 {
            return -1;
        }
        let (mut min, mut max) = ([255i32; 3], [0i32; 3]);
        for &o in &order[start..start + count] {
            let c = self.palette[o as usize];
            for k in 0..3 {
                min[k] = min[k].min(c[k] as i32);
                max[k] = max[k].max(c[k] as i32);
            }
        }
        let range = [max[0] - min[0], max[1] - min[1], max[2] - min[2]];
        let (rs, gs, bs) = (2 * range[0] * range[0], 4 * range[1] * range[1], 3 * range[2] * range[2]);
        let mut axis = if gs > rs { 1 } else { 0 };
        if bs > rs.max(gs) {
            axis = 2;
        }
        let palette = self.palette.clone();
        intro_sort(&mut order[start..start + count], &mut |a: &u8, b: &u8| {
            palette[*a as usize][axis].cmp(&palette[*b as usize][axis])
        });
        let middle = start + count / 2;
        let index = order[middle];
        let sel = self.palette[index as usize];
        let node = *node_count;
        *node_count += 1;
        let left = self.build(order, start, middle - start, node_count);
        let right = self.build(order, middle + 1, start + count - middle - 1, node_count);
        self.nodes[node] = Node {
            r: sel[0] as i32,
            g: sel[1] as i32,
            b: sel[2] as i32,
            index,
            axis: axis as i32,
            left,
            right,
        };
        node as i32
    }

    fn nearest(&mut self, r: i32, g: i32, b: i32) -> u8 {
        let color = (r << 16) | (g << 8) | b;
        let slot = ((color as u32).wrapping_mul(2654435761u32) >> 12) as usize;
        if self.cached_colors[slot] == color {
            return self.cached_indices[slot];
        }
        let mut best = i32::MAX;
        let mut index = 0u8;
        self.search(self.root, r, g, b, &mut best, &mut index);
        self.cached_colors[slot] = color;
        self.cached_indices[slot] = index;
        index
    }

    fn search(&self, pos: i32, r: i32, g: i32, b: i32, best: &mut i32, best_index: &mut u8) {
        if pos < 0 || *best == 0 {
            return;
        }
        let n = self.nodes[pos as usize];
        let (dr, dg, db) = (r - n.r, g - n.g, b - n.b);
        let d = 2 * dr * dr + 4 * dg * dg + 3 * db * db;
        if d < *best {
            *best = d;
            *best_index = n.index;
        }
        let delta = match n.axis {
            0 => dr,
            1 => dg,
            _ => db,
        };
        self.search(if delta <= 0 { n.left } else { n.right }, r, g, b, best, best_index);
        let weight = match n.axis {
            0 => 2,
            1 => 4,
            _ => 3,
        };
        if weight * delta * delta < *best {
            self.search(if delta <= 0 { n.right } else { n.left }, r, g, b, best, best_index);
        }
    }

    fn quantize(&mut self, px: &[Rgba], width: u32, height: u32) -> IndexedImage {
        let (w, h) = (width as usize, height as usize);
        let mut indices = vec![0u8; w * h];
        let len = (w + 2) * 3;
        let mut errors = vec![0i32; len];
        let mut below = vec![0i32; len];
        for y in 0..h {
            let row = y * w;
            for x in 0..w {
                let p = px[row + x];
                if p[3] < 128 {
                    assert!(self.transparent >= 0, "Include transparent frames before building the GIF palette.");
                    indices[row + x] = self.transparent as u8;
                    continue;
                }
                let original = ((p[0] as i32) << 16) | ((p[1] as i32) << 8) | p[2] as i32;
                if let Some(&e) = self.exact.get(&original) {
                    indices[row + x] = e;
                    continue;
                }
                let e = (x + 1) * 3;
                let r = (p[0] as i32 + ((errors[e] + 2) >> 2)).clamp(0, 255);
                let g = (p[1] as i32 + ((errors[e + 1] + 2) >> 2)).clamp(0, 255);
                let b = (p[2] as i32 + ((errors[e + 2] + 2) >> 2)).clamp(0, 255);
                let index = self.nearest(r, g, b);
                indices[row + x] = index;
                let c = self.rgb[index as usize];
                for (k, err) in [r - ((c >> 16) & 255), g - ((c >> 8) & 255), b - (c & 255)].into_iter().enumerate() {
                    errors[e + k + 3] += err * 2;
                    below[e + k - 3] += err;
                    below[e + k] += err;
                }
            }
            std::mem::swap(&mut errors, &mut below);
            below.fill(0);
        }
        IndexedImage::new(width, height, indices, self.palette.clone())
    }
}

// ------------------------------------------------------------ octree, grayscale

/// `SkiaGrayscaleQuantizer` (through `SkiaPaletteQuantizer`'s single pass).
pub fn quantize_grayscale(bmp: &Bitmap) -> IndexedImage {
    let palette: Vec<Rgba> = (0..256u32)
        .map(|i| {
            let v = (i * 0xFF / 255) as u8;
            [v, v, v, 255]
        })
        .collect();
    let indices = straight_pixels(bmp)
        .into_iter()
        .map(|p| {
            let l = p[0] as f64 * 0.299 + p[1] as f64 * 0.587 + p[2] as f64 * 0.114;
            (l + 0.5) as u8
        })
        .collect();
    IndexedImage::new(bmp.width(), bmp.height(), indices, palette)
}

struct OctNode {
    leaf: bool,
    pixel_count: i64,
    red: i64,
    green: i64,
    blue: i64,
    children: Option<[Option<usize>; 8]>,
    palette_index: i32,
    next_reducible: Option<usize>,
}

struct Octree {
    nodes: Vec<OctNode>,
    root: usize,
    leaf_count: i32,
    reducible: [Option<usize>; 9],
    max_bits: i32,
    previous_node: Option<usize>,
    previous_color: i32,
}

const MASK: [i32; 8] = [0x80, 0x40, 0x20, 0x10, 0x08, 0x04, 0x02, 0x01];

/// The `>>` of C#'s `int`: the shift count masked to 5 bits.
fn shr(v: i32, s: i32) -> i32 {
    v >> (s & 31)
}

fn child_index(p: Rgba, level: i32) -> usize {
    let shift = 7 - level;
    let m = MASK[level as usize];
    (shr(p[0] as i32 & m, shift - 2) | shr(p[1] as i32 & m, shift - 1) | shr(p[2] as i32 & m, shift)) as usize
}

impl Octree {
    fn new(max_bits: i32) -> Self {
        let mut t = Octree {
            nodes: Vec::new(),
            root: 0,
            leaf_count: 0,
            reducible: [None; 9],
            max_bits,
            previous_node: None,
            previous_color: 0,
        };
        t.root = t.new_node(0);
        t
    }

    fn new_node(&mut self, level: i32) -> usize {
        let leaf = level == self.max_bits;
        let id = self.nodes.len();
        let next = if leaf {
            self.leaf_count += 1;
            None
        } else {
            let n = self.reducible[level as usize];
            self.reducible[level as usize] = Some(id);
            n
        };
        self.nodes.push(OctNode {
            leaf,
            pixel_count: 0,
            red: 0,
            green: 0,
            blue: 0,
            children: if leaf { None } else { Some([None; 8]) },
            palette_index: 0,
            next_reducible: next,
        });
        id
    }

    fn argb(p: Rgba) -> i32 {
        i32::from_le_bytes([p[2], p[1], p[0], p[3]])
    }

    fn add_color(&mut self, p: Rgba) {
        let argb = Self::argb(p);
        if self.previous_color == argb {
            if let Some(n) = self.previous_node {
                self.increment(n, p);
                return;
            }
        }
        self.previous_color = argb;
        self.add_to(self.root, p, 0);
    }

    fn increment(&mut self, n: usize, p: Rgba) {
        let node = &mut self.nodes[n];
        node.pixel_count += 1;
        node.red += p[0] as i64;
        node.green += p[1] as i64;
        node.blue += p[2] as i64;
    }

    fn add_to(&mut self, n: usize, p: Rgba, level: i32) {
        if self.nodes[n].leaf {
            self.increment(n, p);
            self.previous_node = Some(n);
            return;
        }
        let i = child_index(p, level);
        let child = match self.nodes[n].children.as_ref().unwrap()[i] {
            Some(c) => c,
            None => {
                let c = self.new_node(level + 1);
                self.nodes[n].children.as_mut().unwrap()[i] = Some(c);
                c
            }
        };
        self.add_to(child, p, level + 1);
    }

    fn reduce(&mut self) {
        let mut index = self.max_bits - 1;
        while index > 0 && self.reducible[index as usize].is_none() {
            index -= 1;
        }
        let n = self.reducible[index as usize].expect("reducible node");
        self.reducible[index as usize] = self.nodes[n].next_reducible;
        let (mut r, mut g, mut b, mut count, mut children) = (0i64, 0i64, 0i64, self.nodes[n].pixel_count, 0);
        let kids = self.nodes[n].children.take().unwrap_or([None; 8]);
        for c in kids.into_iter().flatten() {
            r += self.nodes[c].red;
            g += self.nodes[c].green;
            b += self.nodes[c].blue;
            count += self.nodes[c].pixel_count;
            children += 1;
        }
        let node = &mut self.nodes[n];
        node.red = r;
        node.green = g;
        node.blue = b;
        node.pixel_count = count;
        node.leaf = true;
        node.children = Some([None; 8]);
        self.leaf_count -= children - 1;
        self.previous_node = None;
    }

    fn palettize(&mut self, color_count: i32) -> Vec<Rgba> {
        while self.leaf_count > color_count {
            self.reduce();
        }
        let mut palette = Vec::new();
        let mut index = 0;
        self.construct(self.root, &mut palette, &mut index);
        palette
    }

    fn construct(&mut self, n: usize, palette: &mut Vec<Rgba>, index: &mut i32) {
        if self.nodes[n].leaf {
            self.nodes[n].palette_index = *index;
            *index += 1;
            let node = &self.nodes[n];
            let c = node.pixel_count;
            palette.push([(node.red / c) as u8, (node.green / c) as u8, (node.blue / c) as u8, 255]);
        } else if let Some(kids) = self.nodes[n].children {
            for c in kids.into_iter().flatten() {
                self.construct(c, palette, index);
            }
        }
    }

    fn palette_index(&self, n: usize, p: Rgba, level: i32) -> i32 {
        let node = &self.nodes[n];
        if node.leaf {
            return node.palette_index;
        }
        let i = child_index(p, level);
        match node.children.as_ref().unwrap()[i] {
            Some(c) => self.palette_index(c, p, level + 1),
            None => panic!("Didn't expect this!"),
        }
    }
}

/// `SkiaOctreeQuantizer(maxColors, maxColorBits)`.
pub fn quantize_octree(bmp: &Bitmap, max_colors: i32, max_bits: i32) -> IndexedImage {
    let px = straight_pixels(bmp);
    let mut tree = Octree::new(max_bits);
    for &p in &px {
        tree.add_color(p);
    }
    let mut palette = vec![[0, 0, 0, 255]; 256];
    for (i, c) in tree.palettize(max_colors - 1).into_iter().enumerate() {
        palette[i] = c;
    }
    palette[max_colors as usize] = [0, 0, 0, 0];
    let indices = px
        .iter()
        .map(|&p| if p[3] > 0 { tree.palette_index(tree.root, p, 0) as u8 } else { max_colors as u8 })
        .collect();
    IndexedImage::new(bmp.width(), bmp.height(), indices, palette)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn single_colour_gif() {
        let mut b = Bitmap::new(3, 2);
        for y in 0..2 {
            for x in 0..3 {
                b.set_pixel(x, y, [10, 20, 30, 255]);
            }
        }
        let gif = crate::imaging::save_gif(&b, crate::imaging::GifQuality::Adaptive);
        assert!(gif.starts_with(b"GIF89a"));
        assert_eq!(*gif.last().unwrap(), 0x3B);
        let decoded = image::load_from_memory(&gif).unwrap().to_rgba8();
        assert_eq!(decoded.get_pixel(2, 1).0, [10, 20, 30, 255]);
    }
}
