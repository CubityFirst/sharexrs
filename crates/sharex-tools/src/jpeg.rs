//! JPEG encoding as ShareX's SkiaSharp does it: Skia's `SkJpegEncoder` over
//! libjpeg-turbo (built without SIMD, so `DCTELEM` is `int`), byte for byte.
//!
//! - `jpeg_set_defaults` + `jpeg_set_quality(q, force_baseline)`, Skia's
//!   `optimize_coding = TRUE` (Huffman tables generated per image,
//!   `jpeg_gen_optimal_table`), 4:2:0 / 4:2:2 / 4:4:4 as asked,
//! - colour conversion `rgb_ycc_convert` (16-bit fixed-point tables),
//! - downsampling `h2v2_downsample` / `h2v1_downsample` (alternating bias),
//!   with `expand_right_edge` / `expand_bottom_edge` padding,
//! - `jpeg_fdct_islow` and libjpeg-turbo's reciprocal quantizer,
//! - dummy blocks at the right and bottom edges with the last block's DC
//!   (`compress_first_pass`),
//! - markers: SOI, JFIF APP0 (1.01, no density), Skia's sRGB ICC profile in
//!   APP2 when the bitmap has a colour space, DQT, SOF0, DHT, SOS, EOI.

const NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54,
    47, 55, 62, 63,
];

const STD_LUMINANCE: [u32; 64] = [
    16, 11, 10, 16, 24, 40, 51, 61, 12, 12, 14, 19, 26, 58, 60, 55, 14, 13, 16, 24, 40, 57, 69, 56, 14, 17, 22, 29, 51,
    87, 80, 62, 18, 22, 37, 56, 68, 109, 103, 77, 24, 35, 55, 64, 81, 104, 113, 92, 49, 64, 78, 87, 103, 121, 120, 101,
    72, 92, 95, 98, 112, 100, 103, 99,
];

const STD_CHROMINANCE: [u32; 64] = [
    17, 18, 24, 47, 99, 99, 99, 99, 18, 21, 26, 66, 99, 99, 99, 99, 24, 26, 56, 99, 99, 99, 99, 99, 47, 66, 99, 99, 99,
    99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99, 99,
    99, 99, 99, 99, 99, 99,
];

/// The sRGB profile Skia writes (`SkWriteICCProfile` for sRGB), as found in
/// ShareX's JPEGs.
const SKIA_SRGB_ICC: &[u8] = include_bytes!("skia_srgb.icc");

/// `SKJpegEncoderDownsample`.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Subsampling {
    S420,
    S422,
    S444,
}

/// `jpeg_quality_scaling`.
fn quality_scaling(quality: i32) -> u32 {
    let q = quality.clamp(1, 100) as u32;
    if q < 50 { 5000 / q } else { 200 - q * 2 }
}

/// `jpeg_add_quant_table` with `force_baseline`.
fn quant_table(basic: &[u32; 64], scale: u32) -> [u16; 64] {
    let mut t = [0u16; 64];
    for i in 0..64 {
        let v = ((basic[i] as u64 * scale as u64 + 50) / 100).clamp(1, 255);
        t[i] = v as u16;
    }
    t
}

/// `compute_reciprocal` (DCTELEM = int: 32-bit reciprocal, 64-bit product).
#[derive(Clone, Copy)]
struct Divisor {
    recip: u64,
    corr: u64,
    shift: u32,
}

fn compute_reciprocal(divisor: u32) -> Divisor {
    if divisor == 1 {
        return Divisor { recip: 1, corr: 0, shift: 0 };
    }
    let b = 31 - divisor.leading_zeros(); // flss(divisor) - 1
    let mut r = 32 + b;
    let mut fq = (1u64 << r) / divisor as u64;
    let fr = (1u64 << r) % divisor as u64;
    let mut c = (divisor / 2) as u64;
    if fr == 0 {
        fq >>= 1;
        r -= 1;
    } else if fr <= (divisor / 2) as u64 {
        c += 1;
    } else {
        fq += 1;
    }
    Divisor { recip: fq, corr: c, shift: r }
}

/// `quantize`.
fn quantize(work: &[i32; 64], divisors: &[Divisor; 64], out: &mut [i16; 64]) {
    for i in 0..64 {
        let d = divisors[i];
        let temp = work[i];
        let neg = temp < 0;
        let a = temp.unsigned_abs() as u64;
        let product = ((a + d.corr) * d.recip) >> d.shift;
        let v = product as i32;
        out[i] = (if neg { -v } else { v }) as i16;
    }
}

const CONST_BITS: i32 = 13;
const PASS1_BITS: i32 = 2;
const FIX_0_298631336: i32 = 2446;
const FIX_0_390180644: i32 = 3196;
const FIX_0_541196100: i32 = 4433;
const FIX_0_765366865: i32 = 6270;
const FIX_0_899976223: i32 = 7373;
const FIX_1_175875602: i32 = 9633;
const FIX_1_501321110: i32 = 12299;
const FIX_1_847759065: i32 = 15137;
const FIX_1_961570560: i32 = 16069;
const FIX_2_053119869: i32 = 16819;
const FIX_2_562915447: i32 = 20995;
const FIX_3_072711026: i32 = 25172;

fn descale(x: i32, n: i32) -> i32 {
    (x + (1 << (n - 1))) >> n
}

/// `jpeg_fdct_islow`.
fn fdct_islow(d: &mut [i32; 64]) {
    for pass in 0..2 {
        for ctr in 0..8 {
            let (base, step) = if pass == 0 { (ctr * 8, 1) } else { (ctr, 8) };
            let at = |k: usize| base + k * step;
            let tmp0 = d[at(0)] + d[at(7)];
            let tmp7 = d[at(0)] - d[at(7)];
            let tmp1 = d[at(1)] + d[at(6)];
            let tmp6 = d[at(1)] - d[at(6)];
            let tmp2 = d[at(2)] + d[at(5)];
            let tmp5 = d[at(2)] - d[at(5)];
            let tmp3 = d[at(3)] + d[at(4)];
            let tmp4 = d[at(3)] - d[at(4)];
            let tmp10 = tmp0 + tmp3;
            let tmp13 = tmp0 - tmp3;
            let tmp11 = tmp1 + tmp2;
            let tmp12 = tmp1 - tmp2;
            let odd_shift = if pass == 0 { CONST_BITS - PASS1_BITS } else { CONST_BITS + PASS1_BITS };
            if pass == 0 {
                d[at(0)] = (tmp10 + tmp11) << PASS1_BITS;
                d[at(4)] = (tmp10 - tmp11) << PASS1_BITS;
            } else {
                d[at(0)] = descale(tmp10 + tmp11, PASS1_BITS);
                d[at(4)] = descale(tmp10 - tmp11, PASS1_BITS);
            }
            let z1 = (tmp12 + tmp13) * FIX_0_541196100;
            d[at(2)] = descale(z1 + tmp13 * FIX_0_765366865, odd_shift);
            d[at(6)] = descale(z1 + tmp12 * -FIX_1_847759065, odd_shift);
            let z1 = tmp4 + tmp7;
            let z2 = tmp5 + tmp6;
            let z3 = tmp4 + tmp6;
            let z4 = tmp5 + tmp7;
            let z5 = (z3 + z4) * FIX_1_175875602;
            let tmp4 = tmp4 * FIX_0_298631336;
            let tmp5 = tmp5 * FIX_2_053119869;
            let tmp6 = tmp6 * FIX_3_072711026;
            let tmp7 = tmp7 * FIX_1_501321110;
            let z1 = z1 * -FIX_0_899976223;
            let z2 = z2 * -FIX_2_562915447;
            let z3 = z3 * -FIX_1_961570560 + z5;
            let z4 = z4 * -FIX_0_390180644 + z5;
            d[at(7)] = descale(tmp4 + z1 + z3, odd_shift);
            d[at(5)] = descale(tmp5 + z2 + z4, odd_shift);
            d[at(3)] = descale(tmp6 + z2 + z3, odd_shift);
            d[at(1)] = descale(tmp7 + z1 + z4, odd_shift);
        }
    }
}

/// `rgb_ycc_start`'s tables, applied as `rgb_ycc_convert`.
fn rgb_to_ycc(r: u8, g: u8, b: u8) -> (u8, u8, u8) {
    const SCALEBITS: i64 = 16;
    const ONE_HALF: i64 = 1 << (SCALEBITS - 1);
    const CBCR_OFFSET: i64 = 128 << SCALEBITS;
    let fix = |x: f64| (x * (1i64 << SCALEBITS) as f64 + 0.5) as i64;
    let (r, g, b) = (r as i64, g as i64, b as i64);
    let y = (fix(0.29900) * r + fix(0.58700) * g + fix(0.11400) * b + ONE_HALF) >> SCALEBITS;
    let cb = (-fix(0.16874) * r + -fix(0.33126) * g + fix(0.50000) * b + CBCR_OFFSET + ONE_HALF - 1) >> SCALEBITS;
    let cr = (fix(0.50000) * r + CBCR_OFFSET + ONE_HALF - 1 + -fix(0.41869) * g + -fix(0.08131) * b) >> SCALEBITS;
    (y as u8, cb as u8, cr as u8)
}

struct Component {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    width_in_blocks: usize,
    height_in_blocks: usize,
    /// Quantized blocks, row-major over `blocks_wide x blocks_high` (MCU-padded).
    blocks: Vec<[i16; 64]>,
    blocks_wide: usize,
}

fn ceil_div(a: usize, b: usize) -> usize {
    a.div_ceil(b)
}

/// One component's samples for the whole image, padded as libjpeg's
/// preprocessing and downsampling do, ready for the DCT.
fn component_plane(full: &[u8], w: usize, h: usize, hmax: usize, vmax: usize, c: &Component) -> (Vec<u8>, usize, usize) {
    let out_cols = c.width_in_blocks * 8;
    let comp_rows = c.height_in_blocks * 8;
    // Full-resolution rows padded to a multiple of the row group (vmax) by
    // repeating the last row, and to out_cols * h_expand columns by
    // repeating the last column (expand_right_edge).
    let h_expand = hmax / c.h;
    let v_expand = vmax / c.v;
    let in_cols = out_cols * h_expand;
    let padded_rows = ceil_div(h, vmax) * vmax;
    let src = |x: usize, y: usize| full[y.min(h - 1) * w + x.min(w - 1)];
    // Downsampled rows that come from real (or row-group padded) input.
    let produced = padded_rows / v_expand;
    let mut plane = vec![0u8; out_cols * comp_rows];
    for oy in 0..comp_rows.min(produced) {
        let mut bias: u32 = if h_expand == 2 && v_expand == 2 { 1 } else { 0 };
        for ox in 0..out_cols {
            let v = match (h_expand, v_expand) {
                (1, 1) => src(ox, oy) as u32,
                (2, 1) => {
                    let s = src(ox * 2, oy) as u32 + src(ox * 2 + 1, oy) as u32;
                    let v = (s + bias) >> 1;
                    bias ^= 1;
                    v
                }
                (2, 2) => {
                    let s = src(ox * 2, oy * 2) as u32
                        + src(ox * 2 + 1, oy * 2) as u32
                        + src(ox * 2, oy * 2 + 1) as u32
                        + src(ox * 2 + 1, oy * 2 + 1) as u32;
                    let v = (s + bias) >> 2;
                    bias ^= 3;
                    v
                }
                _ => unreachable!("sampling"),
            };
            let _ = in_cols;
            plane[oy * out_cols + ox] = v as u8;
        }
    }
    // expand_bottom_edge on the downsampled rows, to a whole iMCU row.
    let last = comp_rows.min(produced).max(1) - 1;
    for oy in produced..comp_rows {
        let (a, b) = plane.split_at_mut(oy * out_cols);
        b[..out_cols].copy_from_slice(&a[last * out_cols..last * out_cols + out_cols]);
    }
    (plane, out_cols, comp_rows)
}

/// The libjpeg Huffman table for a frequency count (`jpeg_gen_optimal_table`):
/// `(bits[1..=16], huffval)`.
fn gen_optimal_table(freq_in: &[i64; 257]) -> ([u8; 17], Vec<u8>) {
    const MAX_CLEN: usize = 32;
    let mut freq = *freq_in;
    let mut bits = [0i32; MAX_CLEN + 1];
    let mut codesize = [0i32; 257];
    let mut others = [-1i32; 257];
    freq[256] = 1;
    loop {
        let mut c1 = -1i32;
        let mut v = 1_000_000_000i64;
        for i in 0..=256 {
            if freq[i] != 0 && freq[i] <= v {
                v = freq[i];
                c1 = i as i32;
            }
        }
        let mut c2 = -1i32;
        v = 1_000_000_000;
        for i in 0..=256 {
            if freq[i] != 0 && freq[i] <= v && i as i32 != c1 {
                v = freq[i];
                c2 = i as i32;
            }
        }
        if c2 < 0 {
            break;
        }
        let (mut a, mut b) = (c1 as usize, c2 as usize);
        freq[a] += freq[b];
        freq[b] = 0;
        codesize[a] += 1;
        while others[a] >= 0 {
            a = others[a] as usize;
            codesize[a] += 1;
        }
        others[a] = c2;
        codesize[b] += 1;
        while others[b] >= 0 {
            b = others[b] as usize;
            codesize[b] += 1;
        }
    }
    for &cs in codesize.iter() {
        if cs > 0 {
            bits[cs as usize] += 1;
        }
    }
    let mut i = MAX_CLEN;
    while i > 16 {
        while bits[i] > 0 {
            let mut j = i - 2;
            while bits[j] == 0 {
                j -= 1;
            }
            bits[i] -= 2;
            bits[i - 1] += 1;
            bits[j + 1] += 2;
            bits[j] -= 1;
        }
        i -= 1;
    }
    while bits[i] == 0 {
        i -= 1;
    }
    bits[i] -= 1;
    let mut out_bits = [0u8; 17];
    for k in 0..17 {
        out_bits[k] = bits[k] as u8;
    }
    let mut huffval = Vec::new();
    for len in 1..=MAX_CLEN as i32 {
        for (sym, &cs) in codesize.iter().enumerate().take(256) {
            if cs == len {
                huffval.push(sym as u8);
            }
        }
    }
    (out_bits, huffval)
}

/// `jpeg_make_c_derived_tbl`: code and size per symbol.
fn derive(bits: &[u8; 17], huffval: &[u8]) -> ([u32; 256], [u8; 256]) {
    let mut sizes = Vec::new();
    for l in 1..=16 {
        for _ in 0..bits[l] {
            sizes.push(l as u8);
        }
    }
    let mut codes = Vec::with_capacity(sizes.len());
    let mut code = 0u32;
    let mut si = sizes.first().copied().unwrap_or(0);
    let mut p = 0;
    while p < sizes.len() {
        while p < sizes.len() && sizes[p] == si {
            codes.push(code);
            code += 1;
            p += 1;
        }
        code <<= 1;
        si += 1;
    }
    let mut ehufco = [0u32; 256];
    let mut ehufsi = [0u8; 256];
    for (k, &sym) in huffval.iter().enumerate() {
        ehufco[sym as usize] = codes[k];
        ehufsi[sym as usize] = sizes[k];
    }
    (ehufco, ehufsi)
}

fn nbits(v: i32) -> usize {
    (32 - v.unsigned_abs().leading_zeros()) as usize
}

fn count_block(block: &[i16; 64], last_dc: i32, dc: &mut [i64; 257], ac: &mut [i64; 257]) {
    dc[nbits(block[0] as i32 - last_dc)] += 1;
    let mut r = 0;
    for k in 1..64 {
        let t = block[NATURAL_ORDER[k]] as i32;
        if t == 0 {
            r += 1;
        } else {
            while r > 15 {
                ac[0xF0] += 1;
                r -= 16;
            }
            ac[(r << 4) + nbits(t)] += 1;
            r = 0;
        }
    }
    if r > 0 {
        ac[0] += 1;
    }
}

struct BitWriter {
    out: Vec<u8>,
    buffer: u64,
    bits: u32,
}

impl BitWriter {
    fn emit(&mut self, code: u32, size: u32) {
        if size == 0 {
            return;
        }
        self.buffer = (self.buffer << size) | (code as u64 & ((1u64 << size) - 1));
        self.bits += size;
        while self.bits >= 8 {
            let byte = (self.buffer >> (self.bits - 8)) as u8;
            self.out.push(byte);
            if byte == 0xFF {
                self.out.push(0);
            }
            self.bits -= 8;
        }
    }

    fn flush(&mut self) {
        self.emit(0x7F, 7);
        self.buffer = 0;
        self.bits = 0;
    }
}

type Table = ([u32; 256], [u8; 256]);

fn encode_block(w: &mut BitWriter, block: &[i16; 64], last_dc: i32, dc: &Table, ac: &Table) {
    let temp = block[0] as i32 - last_dc;
    let n = nbits(temp);
    w.emit(dc.0[n], dc.1[n] as u32);
    if n > 0 {
        let v = if temp < 0 { temp - 1 } else { temp };
        w.emit(v as u32, n as u32);
    }
    let mut r = 0usize;
    for k in 1..64 {
        let t = block[NATURAL_ORDER[k]] as i32;
        if t == 0 {
            r += 1;
        } else {
            while r > 15 {
                w.emit(ac.0[0xF0], ac.1[0xF0] as u32);
                r -= 16;
            }
            let n = nbits(t);
            let i = (r << 4) + n;
            w.emit(ac.0[i], ac.1[i] as u32);
            let v = if t < 0 { t - 1 } else { t };
            w.emit(v as u32, n as u32);
            r = 0;
        }
    }
    if r > 0 {
        w.emit(ac.0[0], ac.1[0] as u32);
    }
}

fn marker(out: &mut Vec<u8>, m: u8, data: &[u8]) {
    out.extend_from_slice(&[0xFF, m]);
    out.extend_from_slice(&((data.len() + 2) as u16).to_be_bytes());
    out.extend_from_slice(data);
}

/// Encodes straight RGB rows (alpha ignored, as `SKJpegEncoderAlphaOption.Ignore`).
pub fn encode_rgb(rgb: &[u8], width: usize, height: usize, quality: i32, sub: Subsampling, icc: bool) -> Vec<u8> {
    assert!(width > 0 && height > 0 && rgb.len() == width * height * 3);
    let scale = quality_scaling(quality);
    let qt = [quant_table(&STD_LUMINANCE, scale), quant_table(&STD_CHROMINANCE, scale)];
    let divisors: Vec<[Divisor; 64]> =
        qt.iter().map(|t| std::array::from_fn(|i| compute_reciprocal(t[i] as u32 * 8))).collect();
    let (yh, yv) = match sub {
        Subsampling::S420 => (2, 2),
        Subsampling::S422 => (2, 1),
        Subsampling::S444 => (1, 1),
    };
    let (hmax, vmax) = (yh, yv);
    let mut planes = vec![vec![0u8; width * height]; 3];
    for i in 0..width * height {
        let (y, cb, cr) = rgb_to_ycc(rgb[i * 3], rgb[i * 3 + 1], rgb[i * 3 + 2]);
        planes[0][i] = y;
        planes[1][i] = cb;
        planes[2][i] = cr;
    }
    let mcus_x = ceil_div(width, 8 * hmax);
    let mcus_y = ceil_div(height, 8 * vmax);
    let mut comps: Vec<Component> = [(1u8, yh, yv, 0usize), (2, 1, 1, 1), (3, 1, 1, 1)]
        .iter()
        .map(|&(id, h, v, tq)| Component {
            id,
            h,
            v,
            tq,
            width_in_blocks: ceil_div(width * h, 8 * hmax),
            height_in_blocks: ceil_div(height * v, 8 * vmax),
            blocks: Vec::new(),
            blocks_wide: mcus_x * h,
        })
        .collect();
    for (ci, c) in comps.iter_mut().enumerate() {
        let (plane, cols, _rows) = component_plane(&planes[ci], width, height, hmax, vmax, c);
        let bw = c.blocks_wide;
        let bh = mcus_y * c.v;
        let mut blocks = vec![[0i16; 64]; bw * bh];
        for by in 0..bh {
            let real_row = by < c.height_in_blocks;
            for bx in 0..bw {
                if real_row && bx < c.width_in_blocks {
                    let mut work = [0i32; 64];
                    for y in 0..8 {
                        for x in 0..8 {
                            work[y * 8 + x] = plane[(by * 8 + y) * cols + bx * 8 + x] as i32 - 128;
                        }
                    }
                    fdct_islow(&mut work);
                    quantize(&work, &divisors[c.tq], &mut blocks[by * bw + bx]);
                } else if real_row {
                    // Right-edge dummy block: zero, DC of the last real block.
                    let dc = blocks[by * bw + c.width_in_blocks - 1][0];
                    blocks[by * bw + bx][0] = dc;
                }
            }
        }
        // Bottom dummy block rows (last iMCU row), DC per MCU from the row above.
        for by in c.height_in_blocks..bh {
            for mcu in 0..mcus_x {
                let dc = blocks[(by - 1) * bw + mcu * c.h + c.h - 1][0];
                for bi in 0..c.h {
                    blocks[by * bw + mcu * c.h + bi] = [0; 64];
                    blocks[by * bw + mcu * c.h + bi][0] = dc;
                }
            }
        }
        c.blocks = blocks;
    }
    // Pass 1: statistics, in MCU order.
    let mut dc_freq = [[0i64; 257]; 2];
    let mut ac_freq = [[0i64; 257]; 2];
    let mut last = [0i32; 3];
    let each_block = |f: &mut dyn FnMut(usize, &[i16; 64])| {
        for my in 0..mcus_y {
            for mx in 0..mcus_x {
                for (ci, c) in comps.iter().enumerate() {
                    for v in 0..c.v {
                        for h in 0..c.h {
                            let b = &c.blocks[(my * c.v + v) * c.blocks_wide + mx * c.h + h];
                            f(ci, b);
                        }
                    }
                }
            }
        }
    };
    each_block(&mut |ci, b| {
        let t = comps[ci].tq;
        count_block(b, last[ci], &mut dc_freq[t], &mut ac_freq[t]);
        last[ci] = b[0] as i32;
    });
    let tables: Vec<(([u8; 17], Vec<u8>), ([u8; 17], Vec<u8>))> =
        (0..2).map(|t| (gen_optimal_table(&dc_freq[t]), gen_optimal_table(&ac_freq[t]))).collect();
    let derived: Vec<(Table, Table)> =
        tables.iter().map(|(dc, ac)| (derive(&dc.0, &dc.1), derive(&ac.0, &ac.1))).collect();

    // Markers.
    let mut out = vec![0xFF, 0xD8];
    marker(&mut out, 0xE0, &[b'J', b'F', b'I', b'F', 0, 1, 1, 0, 0, 1, 0, 1, 0, 0]);
    if icc {
        let mut d = b"ICC_PROFILE\0".to_vec();
        d.extend_from_slice(&[1, 1]);
        d.extend_from_slice(SKIA_SRGB_ICC);
        marker(&mut out, 0xE2, &d);
    }
    for (t, table) in qt.iter().enumerate() {
        let mut d = vec![t as u8];
        d.extend(NATURAL_ORDER.iter().map(|&n| table[n] as u8));
        marker(&mut out, 0xDB, &d);
    }
    let mut sof = vec![8];
    sof.extend_from_slice(&(height as u16).to_be_bytes());
    sof.extend_from_slice(&(width as u16).to_be_bytes());
    sof.push(3);
    for c in &comps {
        sof.extend_from_slice(&[c.id, ((c.h << 4) | c.v) as u8, c.tq as u8]);
    }
    marker(&mut out, 0xC0, &sof);
    for (t, (dc, ac)) in tables.iter().enumerate() {
        for (class, tbl) in [(0u8, dc), (1u8, ac)] {
            let mut d = vec![(class << 4) | t as u8];
            d.extend_from_slice(&tbl.0[1..17]);
            d.extend_from_slice(&tbl.1);
            marker(&mut out, 0xC4, &d);
        }
    }
    let mut sos = vec![3];
    for c in &comps {
        sos.extend_from_slice(&[c.id, ((c.tq << 4) | c.tq) as u8]);
    }
    sos.extend_from_slice(&[0, 63, 0]);
    marker(&mut out, 0xDA, &sos);
    // Pass 2: entropy-coded data.
    let mut w = BitWriter { out, buffer: 0, bits: 0 };
    let mut last = [0i32; 3];
    each_block(&mut |ci, b| {
        let t = comps[ci].tq;
        encode_block(&mut w, b, last[ci], &derived[t].0, &derived[t].1);
        last[ci] = b[0] as i32;
    });
    w.flush();
    let mut out = w.out;
    out.extend_from_slice(&[0xFF, 0xD9]);
    out
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn fdct_matches_libjpeg() {
        // jpeg-encoder's test vector, taken from libjpeg's jpeg_fdct_islow.
        let input: [i32; 64] = [
            -70, -71, -70, -68, -67, -67, -67, -67, -72, -73, -72, -70, -69, -69, -68, -69, -75, -76, -74, -73, -73, -72,
            -71, -70, -77, -78, -77, -75, -76, -75, -73, -71, -78, -77, -77, -76, -79, -77, -76, -75, -78, -78, -77, -77,
            -77, -77, -78, -77, -79, -79, -78, -78, -78, -78, -79, -78, -80, -79, -78, -78, -81, -80, -78, -76,
        ];
        let expect: [i32; 64] = [
            -4786, -66, 2, -18, 12, 12, 5, -7, 223, -37, -8, 21, 8, 5, -4, 6, 60, 6, -10, 5, 0, -2, -1, 5, 21, 21, -15, 12,
            -2, -7, 1, 0, -2, -5, 16, -15, 0, 5, -4, -8, 0, -7, -4, 6, 7, -4, 5, 4, 3, 0, 1, -5, 0, -1, 4, 1, -5, 7, 0, -3,
            -6, 1, 1, -4,
        ];
        let mut d = input;
        fdct_islow(&mut d);
        assert_eq!(d, expect);
    }

    #[test]
    fn decodes() {
        let rgb: Vec<u8> = (0..17 * 9).flat_map(|i| [(i * 7) as u8, (i * 3) as u8, 200]).collect();
        for sub in [Subsampling::S420, Subsampling::S422, Subsampling::S444] {
            let jpg = encode_rgb(&rgb, 17, 9, 90, sub, true);
            let img = image::load_from_memory(&jpg).unwrap().to_rgb8();
            assert_eq!(img.dimensions(), (17, 9));
        }
    }
}
