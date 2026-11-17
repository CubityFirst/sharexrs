//! JPEG decoding as ShareX's SkCodec does it: libjpeg-turbo (no SIMD) with
//! `JDCT_ISLOW`, fancy upsampling and the `ycc_rgb` tables, so pixels match
//! ShareX's exactly. Baseline and extended sequential Huffman JPEGs; other
//! kinds (progressive, arithmetic, CMYK) return `None` and the caller falls
//! back to the `image` crate.

struct Comp {
    id: u8,
    h: usize,
    v: usize,
    tq: usize,
    td: usize,
    ta: usize,
    width_in_blocks: usize,
    height_in_blocks: usize,
    /// Blocks per row/column in the MCU grid (interleaved scans pad to it).
    bw: usize,
    bh: usize,
    coefs: Vec<[i16; 64]>,
}

#[derive(Clone, Default)]
struct Huff {
    /// (code length, code) -> symbol, via lookup by length.
    maxcode: [i32; 18],
    valoffset: [i32; 17],
    huffval: Vec<u8>,
}

impl Huff {
    fn new(bits: &[u8; 17], huffval: Vec<u8>) -> Huff {
        let mut h = Huff { huffval, ..Default::default() };
        let mut code = 0i32;
        let mut p = 0i32;
        for l in 1..=16 {
            let n = bits[l] as i32;
            if n > 0 {
                h.valoffset[l] = p - code;
                code += n;
                p += n;
                h.maxcode[l] = code - 1;
            } else {
                h.maxcode[l] = -1;
            }
            code <<= 1;
        }
        h.maxcode[17] = i32::MAX;
        h
    }
}

struct Bits<'a> {
    data: &'a [u8],
    pos: usize,
    acc: u32,
    n: u32,
    marker_hit: bool,
}

impl Bits<'_> {
    fn fill(&mut self) {
        while self.n <= 24 {
            let byte = if self.marker_hit || self.pos >= self.data.len() {
                0
            } else {
                let b = self.data[self.pos];
                if b == 0xFF {
                    let next = self.data.get(self.pos + 1).copied().unwrap_or(0);
                    if next == 0 {
                        self.pos += 2;
                        0xFF
                    } else {
                        self.marker_hit = true;
                        0
                    }
                } else {
                    self.pos += 1;
                    b
                }
            };
            self.acc |= (byte as u32) << (24 - self.n);
            self.n += 8;
        }
    }

    fn bit(&mut self) -> i32 {
        self.fill();
        let b = (self.acc >> 31) as i32;
        self.acc <<= 1;
        self.n -= 1;
        b
    }

    fn get(&mut self, k: u32) -> i32 {
        if k == 0 {
            return 0;
        }
        self.fill();
        let v = (self.acc >> (32 - k)) as i32;
        self.acc <<= k;
        self.n -= k;
        v
    }

    fn decode(&mut self, h: &Huff) -> Option<u8> {
        let mut code = self.bit();
        let mut l = 1;
        while code > h.maxcode[l] {
            code = (code << 1) | self.bit();
            l += 1;
            if l > 16 {
                return None;
            }
        }
        h.huffval.get((code + h.valoffset[l]) as usize).copied()
    }

    /// Skips to the next RSTn marker (byte aligned).
    fn restart(&mut self) {
        self.acc = 0;
        self.n = 0;
        self.marker_hit = false;
        while self.pos + 1 < self.data.len() {
            if self.data[self.pos] == 0xFF && (0xD0..=0xD7).contains(&self.data[self.pos + 1]) {
                self.pos += 2;
                return;
            }
            self.pos += 1;
        }
    }
}

/// One block's Huffman-coded coefficients (`decode_mcu`).
fn decode_block(
    bits: &mut Bits,
    pred: &mut i32,
    c: &mut Comp,
    idx: usize,
    dc_tables: &[Option<Huff>; 4],
    ac_tables: &[Option<Huff>; 4],
) -> Option<()> {
    let dc = dc_tables[c.td].as_ref()?;
    let ac = ac_tables[c.ta].as_ref()?;
    let mut block = [0i16; 64];
    let t = bits.decode(dc)? as u32;
    *pred += extend(bits.get(t), t);
    block[0] = *pred as i16;
    let mut z = 1;
    while z < 64 {
        let rs = bits.decode(ac)?;
        let (r, s) = ((rs >> 4) as usize, (rs & 15) as u32);
        if s == 0 {
            if r == 15 {
                z += 16;
                continue;
            }
            break;
        }
        z += r;
        if z > 63 {
            break;
        }
        block[NATURAL_ORDER[z]] = extend(bits.get(s), s) as i16;
        z += 1;
    }
    *c.coefs.get_mut(idx)? = block;
    Some(())
}

fn extend(v: i32, t: u32) -> i32 {
    if t == 0 {
        0
    } else if v < (1 << (t - 1)) {
        v - (1 << t) + 1
    } else {
        v
    }
}

const NATURAL_ORDER: [usize; 64] = [
    0, 1, 8, 16, 9, 2, 3, 10, 17, 24, 32, 25, 18, 11, 4, 5, 12, 19, 26, 33, 40, 48, 41, 34, 27, 20, 13, 6, 7, 14, 21,
    28, 35, 42, 49, 56, 57, 50, 43, 36, 29, 22, 15, 23, 30, 37, 44, 51, 58, 59, 52, 45, 38, 31, 39, 46, 53, 60, 61, 54,
    47, 55, 62, 63,
];

const CONST_BITS: i64 = 13;
const PASS1_BITS: i64 = 2;
const FIX_0_298631336: i64 = 2446;
const FIX_0_390180644: i64 = 3196;
const FIX_0_541196100: i64 = 4433;
const FIX_0_765366865: i64 = 6270;
const FIX_0_899976223: i64 = 7373;
const FIX_1_175875602: i64 = 9633;
const FIX_1_501321110: i64 = 12299;
const FIX_1_847759065: i64 = 15137;
const FIX_1_961570560: i64 = 16069;
const FIX_2_053119869: i64 = 16819;
const FIX_2_562915447: i64 = 20995;
const FIX_3_072711026: i64 = 25172;

fn descale(x: i64, n: i64) -> i64 {
    (x + (1 << (n - 1))) >> n
}

/// `IDCT_range_limit` with `RANGE_MASK`: the masked value as a signed 10-bit
/// number, plus 128, clamped.
fn range_limit(x: i64) -> u8 {
    let v = (x & 1023) as i32;
    let s = if v >= 512 { v - 1024 } else { v };
    (s + 128).clamp(0, 255) as u8
}

/// `jpeg_idct_islow` into an 8x8 sample block.
fn idct_islow(coef: &[i16; 64], q: &[u16; 64], out: &mut [u8], stride: usize) {
    let mut ws = [0i64; 64];
    let deq = |i: usize| coef[i] as i64 * q[i] as i64;
    for c in 0..8 {
        if (1..8).all(|r| coef[r * 8 + c] == 0) {
            let dc = deq(c) << PASS1_BITS;
            for r in 0..8 {
                ws[r * 8 + c] = dc;
            }
            continue;
        }
        let z2 = deq(16 + c);
        let z3 = deq(48 + c);
        let z1 = (z2 + z3) * FIX_0_541196100;
        let tmp2 = z1 + z3 * -FIX_1_847759065;
        let tmp3 = z1 + z2 * FIX_0_765366865;
        let z2 = deq(c);
        let z3 = deq(32 + c);
        let tmp0 = (z2 + z3) << CONST_BITS;
        let tmp1 = (z2 - z3) << CONST_BITS;
        let (tmp10, tmp13, tmp11, tmp12) = (tmp0 + tmp3, tmp0 - tmp3, tmp1 + tmp2, tmp1 - tmp2);
        let (t0, t1, t2, t3) = odd(deq(56 + c), deq(40 + c), deq(24 + c), deq(8 + c));
        let s = CONST_BITS - PASS1_BITS;
        ws[c] = descale(tmp10 + t3, s);
        ws[56 + c] = descale(tmp10 - t3, s);
        ws[8 + c] = descale(tmp11 + t2, s);
        ws[48 + c] = descale(tmp11 - t2, s);
        ws[16 + c] = descale(tmp12 + t1, s);
        ws[40 + c] = descale(tmp12 - t1, s);
        ws[24 + c] = descale(tmp13 + t0, s);
        ws[32 + c] = descale(tmp13 - t0, s);
    }
    for r in 0..8 {
        let w = &ws[r * 8..r * 8 + 8];
        let o = &mut out[r * stride..r * stride + 8];
        // The int workspace holds 32-bit values.
        let w: [i64; 8] = std::array::from_fn(|i| w[i] as i32 as i64);
        if w[1..].iter().all(|&v| v == 0) {
            let dc = range_limit(descale(w[0], PASS1_BITS + 3));
            o.fill(dc);
            continue;
        }
        let z2 = w[2];
        let z3 = w[6];
        let z1 = (z2 + z3) * FIX_0_541196100;
        let tmp2 = z1 + z3 * -FIX_1_847759065;
        let tmp3 = z1 + z2 * FIX_0_765366865;
        let tmp0 = (w[0] + w[4]) << CONST_BITS;
        let tmp1 = (w[0] - w[4]) << CONST_BITS;
        let (tmp10, tmp13, tmp11, tmp12) = (tmp0 + tmp3, tmp0 - tmp3, tmp1 + tmp2, tmp1 - tmp2);
        let (t0, t1, t2, t3) = odd(w[7], w[5], w[3], w[1]);
        let s = CONST_BITS + PASS1_BITS + 3;
        o[0] = range_limit(descale(tmp10 + t3, s));
        o[7] = range_limit(descale(tmp10 - t3, s));
        o[1] = range_limit(descale(tmp11 + t2, s));
        o[6] = range_limit(descale(tmp11 - t2, s));
        o[2] = range_limit(descale(tmp12 + t1, s));
        o[5] = range_limit(descale(tmp12 - t1, s));
        o[3] = range_limit(descale(tmp13 + t0, s));
        o[4] = range_limit(descale(tmp13 - t0, s));
    }
}

/// The IDCT's odd part (inputs y7, y5, y3, y1).
fn odd(tmp0: i64, tmp1: i64, tmp2: i64, tmp3: i64) -> (i64, i64, i64, i64) {
    let z1 = tmp0 + tmp3;
    let z2 = tmp1 + tmp2;
    let z3 = tmp0 + tmp2;
    let z4 = tmp1 + tmp3;
    let z5 = (z3 + z4) * FIX_1_175875602;
    let tmp0 = tmp0 * FIX_0_298631336;
    let tmp1 = tmp1 * FIX_2_053119869;
    let tmp2 = tmp2 * FIX_3_072711026;
    let tmp3 = tmp3 * FIX_1_501321110;
    let z1 = z1 * -FIX_0_899976223;
    let z2 = z2 * -FIX_2_562915447;
    let z3 = z3 * -FIX_1_961570560 + z5;
    let z4 = z4 * -FIX_0_390180644 + z5;
    (tmp0 + z1 + z3, tmp1 + z2 + z4, tmp2 + z2 + z3, tmp3 + z1 + z4)
}

/// Straight RGBA pixels of a sequential Huffman JPEG, or `None`.
pub fn decode(data: &[u8]) -> Option<(u32, u32, Vec<u8>)> {
    if !data.starts_with(&[0xFF, 0xD8]) {
        return None;
    }
    let mut qt = [[0u16; 64]; 4];
    let mut dc_tables: [Option<Huff>; 4] = Default::default();
    let mut ac_tables: [Option<Huff>; 4] = Default::default();
    let mut comps: Vec<Comp> = Vec::new();
    let (mut width, mut height) = (0usize, 0usize);
    let (mut hmax, mut vmax) = (1usize, 1usize);
    let mut restart_interval = 0usize;
    let mut adobe_transform: Option<u8> = None;
    let mut i = 2;
    loop {
        while i < data.len() && data[i] != 0xFF {
            i += 1;
        }
        while i < data.len() && data[i] == 0xFF {
            i += 1;
        }
        let m = *data.get(i)?;
        i += 1;
        if m == 0xD9 {
            break;
        }
        if (0xD0..=0xD7).contains(&m) || m == 0x01 {
            continue;
        }
        let len = u16::from_be_bytes([*data.get(i)?, *data.get(i + 1)?]) as usize;
        let seg = data.get(i + 2..i + len)?;
        match m {
            0xDB => {
                let mut p = 0;
                while p < seg.len() {
                    let pq = seg[p] >> 4;
                    let t = (seg[p] & 15) as usize;
                    p += 1;
                    for k in 0..64 {
                        let v = if pq == 0 {
                            p += 1;
                            seg[p - 1] as u16
                        } else {
                            p += 2;
                            u16::from_be_bytes([seg[p - 2], seg[p - 1]])
                        };
                        qt[t & 3][NATURAL_ORDER[k]] = v;
                    }
                }
            }
            0xC4 => {
                let mut p = 0;
                while p < seg.len() {
                    let class = seg[p] >> 4;
                    let t = (seg[p] & 15) as usize & 3;
                    let mut bits = [0u8; 17];
                    bits[1..17].copy_from_slice(seg.get(p + 1..p + 17)?);
                    let n: usize = bits.iter().map(|&b| b as usize).sum();
                    let vals = seg.get(p + 17..p + 17 + n)?.to_vec();
                    p += 17 + n;
                    let h = Huff::new(&bits, vals);
                    if class == 0 {
                        dc_tables[t] = Some(h);
                    } else {
                        ac_tables[t] = Some(h);
                    }
                }
            }
            0xC0 | 0xC1 => {
                if seg[0] != 8 {
                    return None;
                }
                height = u16::from_be_bytes([seg[1], seg[2]]) as usize;
                width = u16::from_be_bytes([seg[3], seg[4]]) as usize;
                let n = seg[5] as usize;
                if width == 0 || height == 0 || !(n == 1 || n == 3) {
                    return None;
                }
                for k in 0..n {
                    let c = &seg[6 + k * 3..9 + k * 3];
                    comps.push(Comp {
                        id: c[0],
                        h: (c[1] >> 4) as usize,
                        v: (c[1] & 15) as usize,
                        tq: (c[2] & 3) as usize,
                        td: 0,
                        ta: 0,
                        width_in_blocks: 0,
                        height_in_blocks: 0,
                        bw: 0,
                        bh: 0,
                        coefs: Vec::new(),
                    });
                }
                hmax = comps.iter().map(|c| c.h).max()?;
                vmax = comps.iter().map(|c| c.v).max()?;
                let mcus_x = width.div_ceil(8 * hmax);
                let mcus_y = height.div_ceil(8 * vmax);
                for c in &mut comps {
                    if c.h == 0 || c.v == 0 {
                        return None;
                    }
                    c.width_in_blocks = (width * c.h).div_ceil(8 * hmax);
                    c.height_in_blocks = (height * c.v).div_ceil(8 * vmax);
                    c.bw = mcus_x * c.h;
                    c.bh = mcus_y * c.v;
                    c.coefs = vec![[0i16; 64]; c.bw * c.bh];
                }
            }
            0xC2 | 0xC3 | 0xC5..=0xC7 | 0xC9..=0xCB | 0xCD..=0xCF => return None,
            0xDD => restart_interval = u16::from_be_bytes([seg[0], seg[1]]) as usize,
            0xEE if seg.starts_with(b"Adobe") && seg.len() >= 12 => adobe_transform = Some(seg[11]),
            0xDA => {
                let ns = seg[0] as usize;
                let mut scan: Vec<usize> = Vec::new();
                for k in 0..ns {
                    let id = seg[1 + k * 2];
                    let ci = comps.iter().position(|c| c.id == id)?;
                    comps[ci].td = (seg[2 + k * 2] >> 4) as usize & 3;
                    comps[ci].ta = (seg[2 + k * 2] & 15) as usize & 3;
                    scan.push(ci);
                }
                let start = i + len;
                let consumed = decode_scan(&data[start..], &mut comps, &scan, &dc_tables, &ac_tables, restart_interval, width, height, hmax, vmax)?;
                i = start + consumed;
                continue;
            }
            _ => {}
        }
        i += len;
    }
    if comps.is_empty() || comps.iter().any(|c| c.coefs.is_empty()) {
        return None;
    }
    if comps.len() == 3 && adobe_transform == Some(0) {
        return None; // RGB-coded JPEG
    }
    // IDCT every block into a component plane.
    let planes: Vec<(Vec<u8>, usize)> = comps
        .iter()
        .map(|c| {
            let stride = c.bw * 8;
            let mut plane = vec![0u8; stride * c.bh * 8];
            for by in 0..c.bh {
                for bx in 0..c.bw {
                    let off = by * 8 * stride + bx * 8;
                    idct_islow(&c.coefs[by * c.bw + bx], &qt[c.tq], &mut plane[off..], stride);
                }
            }
            (plane, stride)
        })
        .collect();
    // Upsample each component to full resolution.
    let full: Vec<Vec<u8>> = comps
        .iter()
        .zip(&planes)
        .map(|(c, (plane, stride))| upsample(plane, *stride, c, width, height, hmax, vmax))
        .collect();
    let mut rgba = vec![0u8; width * height * 4];
    if comps.len() == 1 {
        for p in 0..width * height {
            let g = full[0][p];
            rgba[p * 4..p * 4 + 4].copy_from_slice(&[g, g, g, 255]);
        }
    } else {
        let t = YccTables::new();
        for p in 0..width * height {
            let (r, g, b) = t.convert(full[0][p], full[1][p], full[2][p]);
            rgba[p * 4..p * 4 + 4].copy_from_slice(&[r, g, b, 255]);
        }
    }
    Some((width as u32, height as u32, rgba))
}

#[allow(clippy::too_many_arguments)]
fn decode_scan(
    data: &[u8],
    comps: &mut [Comp],
    scan: &[usize],
    dc_tables: &[Option<Huff>; 4],
    ac_tables: &[Option<Huff>; 4],
    restart_interval: usize,
    width: usize,
    height: usize,
    hmax: usize,
    vmax: usize,
) -> Option<usize> {
    let mut bits = Bits { data, pos: 0, acc: 0, n: 0, marker_hit: false };
    let mut pred = vec![0i32; scan.len()];
    let mut mcu_count = 0usize;
    let mut restart = |bits: &mut Bits, pred: &mut [i32]| {
        if restart_interval > 0 && mcu_count > 0 && mcu_count % restart_interval == 0 {
            bits.restart();
            pred.iter_mut().for_each(|p| *p = 0);
        }
        mcu_count += 1;
    };
    if scan.len() == 1 {
        let ci = scan[0];
        let (wb, hb) = ((width * comps[ci].h).div_ceil(8 * hmax), (height * comps[ci].v).div_ceil(8 * vmax));
        let bw = comps[ci].bw;
        for by in 0..hb {
            for bx in 0..wb {
                restart(&mut bits, &mut pred);
                decode_block(&mut bits, &mut pred[0], &mut comps[ci], by * bw + bx, dc_tables, ac_tables)?;
            }
        }
    } else {
        let mcus_x = width.div_ceil(8 * hmax);
        let mcus_y = height.div_ceil(8 * vmax);
        for my in 0..mcus_y {
            for mx in 0..mcus_x {
                restart(&mut bits, &mut pred);
                for (k, &ci) in scan.iter().enumerate() {
                    let (h, v, bw) = (comps[ci].h, comps[ci].v, comps[ci].bw);
                    for yy in 0..v {
                        for xx in 0..h {
                            let idx = (my * v + yy) * bw + mx * h + xx;
                            decode_block(&mut bits, &mut pred[k], &mut comps[ci], idx, dc_tables, ac_tables)?;
                        }
                    }
                }
            }
        }
    }
    // Position after the entropy-coded data: the next marker.
    let mut p = bits.pos;
    while p + 1 < data.len() && !(data[p] == 0xFF && data[p + 1] != 0 && !(0xD0..=0xD7).contains(&data[p + 1])) {
        p += 1;
    }
    Some(p)
}

/// Upsamples one component's plane to `width x height` as libjpeg-turbo's
/// `jdsample.c` does (fancy h2v1/h1v2/h2v2 with replicated edge context, plain
/// replication otherwise).
fn upsample(plane: &[u8], stride: usize, c: &Comp, width: usize, height: usize, hmax: usize, vmax: usize) -> Vec<u8> {
    let (hf, vf) = (hmax / c.h, vmax / c.v);
    let dw = (width * c.h).div_ceil(hmax); // downsampled_width
    let dh = (height * c.v).div_ceil(vmax); // downsampled_height
    let at = |x: usize, y: usize| plane[y * stride + x] as i32;
    let mut out = vec![0u8; width * height];
    let fancy = dw > 2;
    match (hf, vf) {
        (1, 1) => {
            for y in 0..height {
                out[y * width..y * width + width].copy_from_slice(&plane[y * stride..y * stride + width]);
            }
        }
        (2, 1) if fancy => {
            let mut row = vec![0u8; dw * 2];
            for y in 0..height {
                row[0] = at(0, y) as u8;
                row[1] = ((at(0, y) * 3 + at(1, y) + 2) >> 2) as u8;
                for x in 1..dw - 1 {
                    let v = at(x, y) * 3;
                    row[x * 2] = ((v + at(x - 1, y) + 1) >> 2) as u8;
                    row[x * 2 + 1] = ((v + at(x + 1, y) + 2) >> 2) as u8;
                }
                let l = dw - 1;
                row[l * 2] = ((at(l, y) * 3 + at(l - 1, y) + 1) >> 2) as u8;
                row[l * 2 + 1] = at(l, y) as u8;
                out[y * width..y * width + width].copy_from_slice(&row[..width]);
            }
        }
        (1, 2) => {
            for oy in 0..height {
                let iy = (oy / 2).min(dh - 1);
                let (other, bias) = if oy % 2 == 0 { (iy.saturating_sub(1), 1) } else { ((iy + 1).min(dh - 1), 2) };
                for x in 0..width {
                    let s = at(x, iy) * 3 + at(x, other);
                    out[oy * width + x] = ((s + bias) >> 2) as u8;
                }
            }
        }
        (2, 2) if fancy => {
            let mut row = vec![0u8; dw * 2];
            for oy in 0..height {
                let iy = (oy / 2).min(dh - 1);
                let other = if oy % 2 == 0 { iy.saturating_sub(1) } else { (iy + 1).min(dh - 1) };
                let col = |x: usize| at(x, iy) * 3 + at(x, other);
                let (mut this, mut next) = (col(0), col(1));
                row[0] = ((this * 4 + 8) >> 4) as u8;
                row[1] = ((this * 3 + next + 7) >> 4) as u8;
                let mut last = this;
                this = next;
                for x in 1..dw - 1 {
                    next = col(x + 1);
                    row[x * 2] = ((this * 3 + last + 8) >> 4) as u8;
                    row[x * 2 + 1] = ((this * 3 + next + 7) >> 4) as u8;
                    last = this;
                    this = next;
                }
                let l = dw - 1;
                row[l * 2] = ((this * 3 + last + 8) >> 4) as u8;
                row[l * 2 + 1] = ((this * 4 + 7) >> 4) as u8;
                out[oy * width..oy * width + width].copy_from_slice(&row[..width]);
            }
        }
        _ => {
            for y in 0..height {
                for x in 0..width {
                    out[y * width + x] = at(x / hf, y / vf) as u8;
                }
            }
        }
    }
    out
}

/// `build_ycc_rgb_table` and `ycc_rgb_convert`.
struct YccTables {
    cr_r: [i32; 256],
    cb_b: [i32; 256],
    cr_g: [i64; 256],
    cb_g: [i64; 256],
}

impl YccTables {
    fn new() -> Self {
        const SCALEBITS: i64 = 16;
        const ONE_HALF: i64 = 1 << (SCALEBITS - 1);
        let fix = |x: f64| (x * (1i64 << SCALEBITS) as f64 + 0.5) as i64;
        let mut t = YccTables { cr_r: [0; 256], cb_b: [0; 256], cr_g: [0; 256], cb_g: [0; 256] };
        for i in 0..256 {
            let x = i as i64 - 128;
            t.cr_r[i] = ((fix(1.40200) * x + ONE_HALF) >> SCALEBITS) as i32;
            t.cb_b[i] = ((fix(1.77200) * x + ONE_HALF) >> SCALEBITS) as i32;
            t.cr_g[i] = -fix(0.71414) * x;
            t.cb_g[i] = -fix(0.34414) * x + ONE_HALF;
        }
        t
    }

    fn convert(&self, y: u8, cb: u8, cr: u8) -> (u8, u8, u8) {
        let y = y as i32;
        let lim = |v: i32| v.clamp(0, 255) as u8;
        let r = lim(y + self.cr_r[cr as usize]);
        let g = lim(y + ((self.cb_g[cb as usize] + self.cr_g[cr as usize]) >> 16) as i32);
        let b = lim(y + self.cb_b[cb as usize]);
        (r, g, b)
    }
}

#[cfg(test)]
mod tests {
    #[test]
    fn round_trips_our_encoder() {
        let rgb: Vec<u8> = (0..23 * 13).flat_map(|i| [(i * 5) as u8, (i * 11) as u8, (255 - i) as u8]).collect();
        for sub in [crate::jpeg::Subsampling::S420, crate::jpeg::Subsampling::S422, crate::jpeg::Subsampling::S444] {
            let jpg = crate::jpeg::encode_rgb(&rgb, 23, 13, 95, sub, false);
            let (w, h, px) = super::decode(&jpg).unwrap();
            assert_eq!((w, h), (23, 13));
            // Another decoder agrees within its IDCT/upsampling rounding.
            let other = image::load_from_memory(&jpg).unwrap().to_rgba8().into_raw();
            let max = px.iter().zip(&other).map(|(a, b)| a.abs_diff(*b)).max().unwrap();
            assert!(max <= 8, "{max}");
        }
    }
}
