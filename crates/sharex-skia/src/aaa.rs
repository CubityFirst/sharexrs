//! Skia's analytic anti-aliasing scan converter (`SkScan_AAAPath.cpp`),
//! with its edges (`SkAnalyticEdge.cpp`, the curve setup of `SkEdge.cpp`)
//! and edge builder (`SkAnalyticEdgeBuilder` in `SkEdgeBuilder.cpp`),
//! transcribed so the coverage it produces is bit-identical.
//!
//! Edges live in a `Vec` and link to each other by index, standing in for
//! Skia's intrusive linked list (`NONE` is its nullptr).

use crate::blitter::Blitter;
use crate::fixed::*;
use crate::geom::{self, IRect, Point};
use crate::path::{Edge as PathEdge, FillType, LINE_SEGMENT_MASK, Path};

const NONE: usize = usize::MAX;
const DEFAULT_ACCURACY: i32 = 2;
const MAX_COEFF_SHIFT: i32 = 6;

#[derive(Clone, Copy, PartialEq, Eq, Debug, Default)]
pub(crate) enum EdgeType {
    #[default]
    Line,
    Quad,
    Cubic,
}

#[derive(Clone, Default, Debug)]
pub(crate) struct AEdge {
    // SkEdge (the supersampling scan converter's edges)
    pub(crate) first_y: i32,
    pub(crate) last_y: i32,

    pub(crate) next: usize,
    pub(crate) prev: usize,
    pub(crate) rite_e: usize,

    pub(crate) x: Fixed,
    pub(crate) dx: Fixed,
    pub(crate) upper_x: Fixed,
    pub(crate) y: Fixed,
    pub(crate) upper_y: Fixed,
    pub(crate) lower_y: Fixed,
    pub(crate) dy: Fixed,

    pub(crate) saved_x: Fixed,
    pub(crate) saved_y: Fixed,
    pub(crate) saved_dy: Fixed,

    pub(crate) edge_type: EdgeType,
    pub(crate) curve_count: i8,
    pub(crate) curve_shift: u8,
    pub(crate) cubic_dshift: u8,
    pub(crate) winding: i8,

    // SkQuadraticEdge
    pub(crate) qx: Fixed,
    pub(crate) qy: Fixed,
    pub(crate) qdx: Fixed,
    pub(crate) qdy: Fixed,
    pub(crate) qddx: Fixed,
    pub(crate) qddy: Fixed,
    pub(crate) qlast_x: Fixed,
    pub(crate) qlast_y: Fixed,
    pub(crate) q_curve_shift: u8,
    pub(crate) snapped_x: Fixed,
    pub(crate) snapped_y: Fixed,

    // SkCubicEdge
    pub(crate) cx: Fixed,
    pub(crate) cy: Fixed,
    pub(crate) cdx: Fixed,
    pub(crate) cdy: Fixed,
    pub(crate) cddx: Fixed,
    pub(crate) cddy: Fixed,
    pub(crate) cdddx: Fixed,
    pub(crate) cdddy: Fixed,
    pub(crate) clast_x: Fixed,
    pub(crate) clast_y: Fixed,
    pub(crate) c_curve_shift: u8,
    pub(crate) c_cubic_dshift: u8,
}

// --- SkAnalyticEdge.cpp ----------------------------------------------------

#[rustfmt::skip]
static INVERSE_TABLE: [i32; 1025] = [
    -4096, -4100, -4104, -4108, -4112, -4116, -4120, -4124, -4128, -4132, -4136,
    -4140, -4144, -4148, -4152, -4156, -4161, -4165, -4169, -4173, -4177, -4181,
    -4185, -4190, -4194, -4198, -4202, -4206, -4211, -4215, -4219, -4223, -4228,
    -4232, -4236, -4240, -4245, -4249, -4253, -4258, -4262, -4266, -4271, -4275,
    -4279, -4284, -4288, -4293, -4297, -4301, -4306, -4310, -4315, -4319, -4324,
    -4328, -4332, -4337, -4341, -4346, -4350, -4355, -4359, -4364, -4369, -4373,
    -4378, -4382, -4387, -4391, -4396, -4401, -4405, -4410, -4415, -4419, -4424,
    -4429, -4433, -4438, -4443, -4447, -4452, -4457, -4462, -4466, -4471, -4476,
    -4481, -4485, -4490, -4495, -4500, -4505, -4510, -4514, -4519, -4524, -4529,
    -4534, -4539, -4544, -4549, -4554, -4559, -4563, -4568, -4573, -4578, -4583,
    -4588, -4593, -4599, -4604, -4609, -4614, -4619, -4624, -4629, -4634, -4639,
    -4644, -4650, -4655, -4660, -4665, -4670, -4675, -4681, -4686, -4691, -4696,
    -4702, -4707, -4712, -4718, -4723, -4728, -4733, -4739, -4744, -4750, -4755,
    -4760, -4766, -4771, -4777, -4782, -4788, -4793, -4798, -4804, -4809, -4815,
    -4821, -4826, -4832, -4837, -4843, -4848, -4854, -4860, -4865, -4871, -4877,
    -4882, -4888, -4894, -4899, -4905, -4911, -4917, -4922, -4928, -4934, -4940,
    -4946, -4951, -4957, -4963, -4969, -4975, -4981, -4987, -4993, -4999, -5005,
    -5011, -5017, -5023, -5029, -5035, -5041, -5047, -5053, -5059, -5065, -5071,
    -5077, -5084, -5090, -5096, -5102, -5108, -5115, -5121, -5127, -5133, -5140,
    -5146, -5152, -5159, -5165, -5171, -5178, -5184, -5190, -5197, -5203, -5210,
    -5216, -5223, -5229, -5236, -5242, -5249, -5256, -5262, -5269, -5275, -5282,
    -5289, -5295, -5302, -5309, -5315, -5322, -5329, -5336, -5343, -5349, -5356,
    -5363, -5370, -5377, -5384, -5391, -5398, -5405, -5412, -5418, -5426, -5433,
    -5440, -5447, -5454, -5461, -5468, -5475, -5482, -5489, -5497, -5504, -5511,
    -5518, -5526, -5533, -5540, -5548, -5555, -5562, -5570, -5577, -5584, -5592,
    -5599, -5607, -5614, -5622, -5629, -5637, -5645, -5652, -5660, -5667, -5675,
    -5683, -5691, -5698, -5706, -5714, -5722, -5729, -5737, -5745, -5753, -5761,
    -5769, -5777, -5785, -5793, -5801, -5809, -5817, -5825, -5833, -5841, -5849,
    -5857, -5866, -5874, -5882, -5890, -5899, -5907, -5915, -5924, -5932, -5940,
    -5949, -5957, -5966, -5974, -5983, -5991, -6000, -6009, -6017, -6026, -6034,
    -6043, -6052, -6061, -6069, -6078, -6087, -6096, -6105, -6114, -6123, -6132,
    -6141, -6150, -6159, -6168, -6177, -6186, -6195, -6204, -6213, -6223, -6232,
    -6241, -6250, -6260, -6269, -6278, -6288, -6297, -6307, -6316, -6326, -6335,
    -6345, -6355, -6364, -6374, -6384, -6393, -6403, -6413, -6423, -6432, -6442,
    -6452, -6462, -6472, -6482, -6492, -6502, -6512, -6523, -6533, -6543, -6553,
    -6563, -6574, -6584, -6594, -6605, -6615, -6626, -6636, -6647, -6657, -6668,
    -6678, -6689, -6700, -6710, -6721, -6732, -6743, -6754, -6765, -6775, -6786,
    -6797, -6808, -6820, -6831, -6842, -6853, -6864, -6875, -6887, -6898, -6909,
    -6921, -6932, -6944, -6955, -6967, -6978, -6990, -7002, -7013, -7025, -7037,
    -7049, -7061, -7073, -7084, -7096, -7108, -7121, -7133, -7145, -7157, -7169,
    -7182, -7194, -7206, -7219, -7231, -7244, -7256, -7269, -7281, -7294, -7307,
    -7319, -7332, -7345, -7358, -7371, -7384, -7397, -7410, -7423, -7436, -7449,
    -7463, -7476, -7489, -7503, -7516, -7530, -7543, -7557, -7570, -7584, -7598,
    -7612, -7626, -7639, -7653, -7667, -7681, -7695, -7710, -7724, -7738, -7752,
    -7767, -7781, -7796, -7810, -7825, -7839, -7854, -7869, -7884, -7898, -7913,
    -7928, -7943, -7958, -7973, -7989, -8004, -8019, -8035, -8050, -8065, -8081,
    -8097, -8112, -8128, -8144, -8160, -8176, -8192, -8208, -8224, -8240, -8256,
    -8272, -8289, -8305, -8322, -8338, -8355, -8371, -8388, -8405, -8422, -8439,
    -8456, -8473, -8490, -8507, -8525, -8542, -8559, -8577, -8594, -8612, -8630,
    -8648, -8665, -8683, -8701, -8719, -8738, -8756, -8774, -8793, -8811, -8830,
    -8848, -8867, -8886, -8905, -8924, -8943, -8962, -8981, -9000, -9020, -9039,
    -9058, -9078, -9098, -9118, -9137, -9157, -9177, -9198, -9218, -9238, -9258,
    -9279, -9300, -9320, -9341, -9362, -9383, -9404, -9425, -9446, -9467, -9489,
    -9510, -9532, -9554, -9576, -9597, -9619, -9642, -9664, -9686, -9709, -9731,
    -9754, -9776, -9799, -9822, -9845, -9868, -9892, -9915, -9939, -9962, -9986,
    -10010, -10034, -10058, -10082, -10106, -10131, -10155, -10180, -10205, -10230,
    -10255, -10280, -10305, -10330, -10356, -10381, -10407, -10433, -10459, -10485,
    -10512, -10538, -10564, -10591, -10618, -10645, -10672, -10699, -10727, -10754,
    -10782, -10810, -10837, -10866, -10894, -10922, -10951, -10979, -11008, -11037,
    -11066, -11096, -11125, -11155, -11184, -11214, -11244, -11275, -11305, -11335,
    -11366, -11397, -11428, -11459, -11491, -11522, -11554, -11586, -11618, -11650,
    -11683, -11715, -11748, -11781, -11814, -11848, -11881, -11915, -11949, -11983,
    -12018, -12052, -12087, -12122, -12157, -12192, -12228, -12264, -12300, -12336,
    -12372, -12409, -12446, -12483, -12520, -12557, -12595, -12633, -12671, -12710,
    -12748, -12787, -12826, -12865, -12905, -12945, -12985, -13025, -13066, -13107,
    -13148, -13189, -13231, -13273, -13315, -13357, -13400, -13443, -13486, -13530,
    -13573, -13617, -13662, -13706, -13751, -13797, -13842, -13888, -13934, -13981,
    -14027, -14074, -14122, -14169, -14217, -14266, -14315, -14364, -14413, -14463,
    -14513, -14563, -14614, -14665, -14716, -14768, -14820, -14873, -14926, -14979,
    -15033, -15087, -15141, -15196, -15252, -15307, -15363, -15420, -15477, -15534,
    -15592, -15650, -15709, -15768, -15827, -15887, -15947, -16008, -16070, -16131,
    -16194, -16256, -16320, -16384, -16448, -16513, -16578, -16644, -16710, -16777,
    -16844, -16912, -16980, -17050, -17119, -17189, -17260, -17331, -17403, -17476,
    -17549, -17623, -17697, -17772, -17848, -17924, -18001, -18078, -18157, -18236,
    -18315, -18396, -18477, -18558, -18641, -18724, -18808, -18893, -18978, -19065,
    -19152, -19239, -19328, -19418, -19508, -19599, -19691, -19784, -19878, -19972,
    -20068, -20164, -20262, -20360, -20460, -20560, -20661, -20763, -20867, -20971,
    -21076, -21183, -21290, -21399, -21509, -21620, -21732, -21845, -21959, -22075,
    -22192, -22310, -22429, -22550, -22671, -22795, -22919, -23045, -23172, -23301,
    -23431, -23563, -23696, -23831, -23967, -24105, -24244, -24385, -24528, -24672,
    -24818, -24966, -25115, -25266, -25420, -25575, -25731, -25890, -26051, -26214,
    -26379, -26546, -26715, -26886, -27060, -27235, -27413, -27594, -27776, -27962,
    -28149, -28339, -28532, -28728, -28926, -29127, -29330, -29537, -29746, -29959,
    -30174, -30393, -30615, -30840, -31068, -31300, -31536, -31775, -32017, -32263,
    -32513, -32768, -33026, -33288, -33554, -33825, -34100, -34379, -34663, -34952,
    -35246, -35544, -35848, -36157, -36472, -36792, -37117, -37449, -37786, -38130,
    -38479, -38836, -39199, -39568, -39945, -40329, -40721, -41120, -41527, -41943,
    -42366, -42799, -43240, -43690, -44150, -44620, -45100, -45590, -46091, -46603,
    -47127, -47662, -48210, -48770, -49344, -49932, -50533, -51150, -51781, -52428,
    -53092, -53773, -54471, -55188, -55924, -56679, -57456, -58254, -59074, -59918,
    -60787, -61680, -62601, -63550, -64527, -65536, -66576, -67650, -68759, -69905,
    -71089, -72315, -73584, -74898, -76260, -77672, -79137, -80659, -82241, -83886,
    -85598, -87381, -89240, -91180, -93206, -95325, -97541, -99864, -102300,
    -104857, -107546, -110376, -113359, -116508, -119837, -123361, -127100, -131072,
    -135300, -139810, -144631, -149796, -155344, -161319, -167772, -174762, -182361,
    -190650, -199728, -209715, -220752, -233016, -246723, -262144, -279620, -299593,
    -322638, -349525, -381300, -419430, -466033, -524288, -599186, -699050, -838860,
    -1048576, -1398101, -2097152, -4194304, 0,
];

const INVERSE_TABLE_SIZE: i32 = 1024;

fn quick_inverse(x: FDot6) -> Fixed {
    const LAST: i32 = INVERSE_TABLE_SIZE;
    if x > 0 {
        -INVERSE_TABLE[(LAST - x) as usize]
    } else {
        INVERSE_TABLE[(LAST + x) as usize]
    }
}

fn quick_div(a: FDot6, b: FDot6) -> Fixed {
    const MIN_BITS: i32 = 3;
    const MAX_BITS: i32 = 31;
    const MAX_ABS_A: i32 = 1 << (MAX_BITS - (22 - MIN_BITS));
    let abs_a = a.wrapping_abs();
    let abs_b = b.wrapping_abs();
    if abs_b >= (1 << MIN_BITS) && abs_b < INVERSE_TABLE_SIZE && abs_a < MAX_ABS_A {
        return a.wrapping_mul(quick_inverse(b)) >> 6;
    }
    fdot6_div(a, b)
}

#[inline]
fn snap_y(y: Fixed) -> Fixed {
    const ACC: i32 = DEFAULT_ACCURACY;
    ((((y as u32).wrapping_add((FIXED1 >> (ACC + 1)) as u32)) >> (16 - ACC)) << (16 - ACC)) as i32
}

impl AEdge {
    fn go_y(&mut self, y: Fixed) {
        if y == self.y.wrapping_add(FIXED1) {
            self.x = self.x.wrapping_add(self.dx);
            self.y = y;
        } else if y != self.y {
            self.x = self
                .upper_x
                .wrapping_add(fixed_mul(self.dx, y.wrapping_sub(self.upper_y)));
            self.y = y;
        }
    }

    fn go_y_shift(&mut self, y: Fixed, y_shift: i32) {
        self.y = y;
        self.x = self.x.wrapping_add(self.dx >> y_shift);
    }

    fn save_xy(&mut self, x: Fixed, y: Fixed, dy: Fixed) {
        self.saved_x = x;
        self.saved_y = y;
        self.saved_dy = dy;
    }

    fn set_line(&mut self, p0: Point, p1: Point) -> bool {
        self.rite_e = NONE;
        let acc = DEFAULT_ACCURACY;
        let mul = (1 << acc) as f32;
        let mut x0 = fdot6_to_fixed(scalar_to_fdot6(p0.x * mul)) >> acc;
        let mut y0 = snap_y(fdot6_to_fixed(scalar_to_fdot6(p0.y * mul)) >> acc);
        let mut x1 = fdot6_to_fixed(scalar_to_fdot6(p1.x * mul)) >> acc;
        let mut y1 = snap_y(fdot6_to_fixed(scalar_to_fdot6(p1.y * mul)) >> acc);

        let mut winding = 1;
        if y0 > y1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            winding = -1;
        }

        let dy = fixed_to_fdot6(y1.wrapping_sub(y0));
        if dy == 0 {
            return false;
        }
        let dx = fixed_to_fdot6(x1.wrapping_sub(x0));
        let slope = quick_div(dx, dy);
        let abs_slope = slope.wrapping_abs();

        self.x = x0;
        self.dx = slope;
        self.upper_x = x0;
        self.y = y0;
        self.upper_y = y0;
        self.lower_y = y1;
        self.dy = if dx == 0 || slope == 0 {
            MAX_S32
        } else if abs_slope < INVERSE_TABLE_SIZE {
            quick_inverse(abs_slope)
        } else {
            quick_div(dy, dx).wrapping_abs()
        };
        self.edge_type = EdgeType::Line;
        self.curve_count = 0;
        self.winding = winding;
        self.curve_shift = 0;
        true
    }

    fn update_line(
        &mut self,
        mut x0: Fixed,
        mut y0: Fixed,
        mut x1: Fixed,
        mut y1: Fixed,
        slope: Fixed,
    ) -> bool {
        if y0 > y1 {
            std::mem::swap(&mut x0, &mut x1);
            std::mem::swap(&mut y0, &mut y1);
            self.winding = -self.winding;
        }
        let dx = fixed_to_fdot6(x1.wrapping_sub(x0));
        let dy = fixed_to_fdot6(y1.wrapping_sub(y0));
        if dy == 0 {
            return false;
        }
        let abs_slope = fixed_to_fdot6(slope).wrapping_abs();
        self.x = x0;
        self.dx = slope;
        self.upper_x = x0;
        self.y = y0;
        self.upper_y = y0;
        self.lower_y = y1;
        self.dy = if dx == 0 || slope == 0 {
            MAX_S32
        } else if abs_slope < INVERSE_TABLE_SIZE {
            quick_inverse(abs_slope)
        } else {
            quick_div(dy, dx).wrapping_abs()
        };
        true
    }

    fn update(&mut self, _last_y: Fixed) -> bool {
        if self.curve_count < 0 {
            self.update_cubic(true)
        } else if self.curve_count > 0 {
            self.update_quadratic()
        } else {
            false
        }
    }

    // SkQuadraticEdge::setQuadraticWithoutUpdate
    pub(crate) fn set_quadratic_without_update(&mut self, pts: &[Point; 3], shift_up: i32) -> bool {
        let scale = (1 << (shift_up + 6)) as f32;
        let mut x0 = (pts[0].x * scale) as i32;
        let mut y0 = (pts[0].y * scale) as i32;
        let x1 = (pts[1].x * scale) as i32;
        let y1 = (pts[1].y * scale) as i32;
        let mut x2 = (pts[2].x * scale) as i32;
        let mut y2 = (pts[2].y * scale) as i32;

        let mut winding = 1;
        if y0 > y2 {
            std::mem::swap(&mut x0, &mut x2);
            std::mem::swap(&mut y0, &mut y2);
            winding = -1;
        }

        let top = fdot6_round(y0);
        let bot = fdot6_round(y2);
        if top == bot {
            return false;
        }

        let mut shift;
        {
            let dx = (left_shift(x1, 1).wrapping_sub(x0).wrapping_sub(x2)) >> 2;
            let dy = (left_shift(y1, 1).wrapping_sub(y0).wrapping_sub(y2)) >> 2;
            shift = diff_to_shift(dx, dy, shift_up);
        }
        if shift == 0 {
            shift = 1;
        } else if shift > MAX_COEFF_SHIFT {
            shift = MAX_COEFF_SHIFT;
        }

        self.winding = winding;
        self.edge_type = EdgeType::Quad;
        self.curve_count = (1i32 << shift) as i8;
        self.q_curve_shift = (shift - 1) as u8;

        let a = fdot6_to_fixed_div2(x0.wrapping_sub(x1).wrapping_sub(x1).wrapping_add(x2));
        let b = fdot6_to_fixed(x1.wrapping_sub(x0));
        self.qx = fdot6_to_fixed(x0);
        self.qdx = b.wrapping_add(a >> shift);
        self.qddx = a >> (shift - 1);

        let a = fdot6_to_fixed_div2(y0.wrapping_sub(y1).wrapping_sub(y1).wrapping_add(y2));
        let b = fdot6_to_fixed(y1.wrapping_sub(y0));
        self.qy = fdot6_to_fixed(y0);
        self.qdy = b.wrapping_add(a >> shift);
        self.qddy = a >> (shift - 1);

        self.qlast_x = fdot6_to_fixed(x2);
        self.qlast_y = fdot6_to_fixed(y2);
        true
    }

    fn set_quadratic(&mut self, pts: &[Point; 3]) -> bool {
        self.rite_e = NONE;
        if !self.set_quadratic_without_update(pts, DEFAULT_ACCURACY) {
            return false;
        }
        let acc = DEFAULT_ACCURACY;
        self.qx >>= acc;
        self.qy >>= acc;
        self.qdx >>= acc;
        self.qdy >>= acc;
        self.qddx >>= acc;
        self.qddy >>= acc;
        self.qlast_x >>= acc;
        self.qlast_y >>= acc;
        self.qy = snap_y(self.qy);
        self.qlast_y = snap_y(self.qlast_y);

        self.edge_type = EdgeType::Quad;
        self.curve_shift = self.q_curve_shift;

        self.snapped_x = self.qx;
        self.snapped_y = self.qy;

        self.update_quadratic()
    }

    fn update_quadratic(&mut self) -> bool {
        let mut success = false;
        let mut count = self.curve_count as i32;
        let mut oldx = self.qx;
        let mut oldy = self.qy;
        let mut dx = self.qdx;
        let mut dy = self.qdy;
        let (mut newx, mut newy, mut new_snapped_x, mut new_snapped_y);
        let shift = self.curve_shift as i32;

        loop {
            let slope;
            count -= 1;
            if count > 0 {
                newx = oldx.wrapping_add(dx >> shift);
                newy = oldy.wrapping_add(dy >> shift);
                if (dy >> shift).wrapping_abs() >= FIXED1 * 2 {
                    let diff_y = fixed_to_fdot6(newy.wrapping_sub(self.snapped_y));
                    slope = if diff_y != 0 {
                        quick_div(fixed_to_fdot6(newx.wrapping_sub(self.snapped_x)), diff_y)
                    } else {
                        MAX_S32
                    };
                    new_snapped_y = self.qlast_y.min(fixed_round_to_fixed(newy));
                    new_snapped_x =
                        newx.wrapping_sub(fixed_mul(slope, newy.wrapping_sub(new_snapped_y)));
                } else {
                    new_snapped_y = self.qlast_y.min(snap_y(newy));
                    new_snapped_x = newx;
                    let diff_y = fixed_to_fdot6(new_snapped_y.wrapping_sub(self.snapped_y));
                    slope = if diff_y != 0 {
                        quick_div(fixed_to_fdot6(newx.wrapping_sub(self.snapped_x)), diff_y)
                    } else {
                        MAX_S32
                    };
                }
                dx = dx.wrapping_add(self.qddx);
                dy = dy.wrapping_add(self.qddy);
            } else {
                newx = self.qlast_x;
                newy = self.qlast_y;
                new_snapped_y = newy;
                new_snapped_x = newx;
                let diff_y = newy.wrapping_sub(self.snapped_y) >> 10;
                slope = if diff_y != 0 {
                    quick_div(newx.wrapping_sub(self.snapped_x) >> 10, diff_y)
                } else {
                    MAX_S32
                };
            }
            if slope < MAX_S32 {
                success = self.update_line(
                    self.snapped_x,
                    self.snapped_y,
                    new_snapped_x,
                    new_snapped_y,
                    slope,
                );
            }
            oldx = newx;
            oldy = newy;
            if !(count > 0 && !success) {
                break;
            }
        }

        self.qx = newx;
        self.qy = newy;
        self.qdx = dx;
        self.qdy = dy;
        self.snapped_x = new_snapped_x;
        self.snapped_y = new_snapped_y;
        self.curve_count = count as i8;
        let _ = (oldx, oldy);
        success
    }

    fn keep_continuous_quad(&mut self) {
        self.snapped_x = self.x;
        self.snapped_y = self.y;
    }

    // SkCubicEdge::setCubicWithoutUpdate
    pub(crate) fn set_cubic_without_update(
        &mut self,
        pts: &[Point; 4],
        shift_up: i32,
        sort_y: bool,
    ) -> bool {
        let scale = (1 << (shift_up + 6)) as f32;
        let mut x0 = (pts[0].x * scale) as i32;
        let mut y0 = (pts[0].y * scale) as i32;
        let mut x1 = (pts[1].x * scale) as i32;
        let mut y1 = (pts[1].y * scale) as i32;
        let mut x2 = (pts[2].x * scale) as i32;
        let mut y2 = (pts[2].y * scale) as i32;
        let mut x3 = (pts[3].x * scale) as i32;
        let mut y3 = (pts[3].y * scale) as i32;

        let mut winding = 1;
        if sort_y && y0 > y3 {
            std::mem::swap(&mut x0, &mut x3);
            std::mem::swap(&mut x1, &mut x2);
            std::mem::swap(&mut y0, &mut y3);
            std::mem::swap(&mut y1, &mut y2);
            winding = -1;
        }

        let top = fdot6_round(y0);
        let bot = fdot6_round(y3);
        if sort_y && top == bot {
            return false;
        }

        let mut shift;
        {
            let dx = cubic_delta_from_line(x0, x1, x2, x3);
            let dy = cubic_delta_from_line(y0, y1, y2, y3);
            shift = diff_to_shift(dx, dy, 2) + 1;
        }
        if shift > MAX_COEFF_SHIFT {
            shift = MAX_COEFF_SHIFT;
        }

        let mut up_shift = 6;
        let mut down_shift = shift + up_shift - 10;
        if down_shift < 0 {
            down_shift = 0;
            up_shift = 10 - shift;
        }

        self.winding = winding;
        self.edge_type = EdgeType::Cubic;
        self.curve_count = left_shift(-1, shift) as i8;
        self.c_curve_shift = shift as u8;
        self.c_cubic_dshift = down_shift as u8;

        let b = left_shift(3i32.wrapping_mul(x1.wrapping_sub(x0)), up_shift);
        let c = left_shift(
            3i32.wrapping_mul(x0.wrapping_sub(x1).wrapping_sub(x1).wrapping_add(x2)),
            up_shift,
        );
        let d = left_shift(
            x3.wrapping_add(3i32.wrapping_mul(x1.wrapping_sub(x2)))
                .wrapping_sub(x0),
            up_shift,
        );

        self.cx = fdot6_to_fixed(x0);
        self.cdx = b.wrapping_add(c >> shift).wrapping_add(d >> (2 * shift));
        self.cddx = 2i32
            .wrapping_mul(c)
            .wrapping_add(3i32.wrapping_mul(d) >> (shift - 1));
        self.cdddx = 3i32.wrapping_mul(d) >> (shift - 1);

        let b = left_shift(3i32.wrapping_mul(y1.wrapping_sub(y0)), up_shift);
        let c = left_shift(
            3i32.wrapping_mul(y0.wrapping_sub(y1).wrapping_sub(y1).wrapping_add(y2)),
            up_shift,
        );
        let d = left_shift(
            y3.wrapping_add(3i32.wrapping_mul(y1.wrapping_sub(y2)))
                .wrapping_sub(y0),
            up_shift,
        );

        self.cy = fdot6_to_fixed(y0);
        self.cdy = b.wrapping_add(c >> shift).wrapping_add(d >> (2 * shift));
        self.cddy = 2i32
            .wrapping_mul(c)
            .wrapping_add(3i32.wrapping_mul(d) >> (shift - 1));
        self.cdddy = 3i32.wrapping_mul(d) >> (shift - 1);

        self.clast_x = fdot6_to_fixed(x3);
        self.clast_y = fdot6_to_fixed(y3);
        true
    }

    fn set_cubic(&mut self, pts: &[Point; 4], sort_y: bool) -> bool {
        self.rite_e = NONE;
        if !self.set_cubic_without_update(pts, DEFAULT_ACCURACY, sort_y) {
            return false;
        }
        let acc = DEFAULT_ACCURACY;
        self.cx >>= acc;
        self.cy >>= acc;
        self.cdx >>= acc;
        self.cdy >>= acc;
        self.cddx >>= acc;
        self.cddy >>= acc;
        self.cdddx >>= acc;
        self.cdddy >>= acc;
        self.clast_x >>= acc;
        self.clast_y >>= acc;
        self.cy = snap_y(self.cy);
        self.clast_y = snap_y(self.clast_y);

        self.edge_type = EdgeType::Cubic;
        self.curve_shift = self.c_curve_shift;
        self.cubic_dshift = self.c_cubic_dshift;

        self.snapped_y = self.cy;

        self.update_cubic(sort_y)
    }

    fn update_cubic(&mut self, sort_y: bool) -> bool {
        let mut success;
        let mut count = self.curve_count as i32;
        let mut oldx = self.cx;
        let mut oldy = self.cy;
        let (mut newx, mut newy);
        let ddshift = self.curve_shift as i32;
        let dshift = self.cubic_dshift as i32;

        loop {
            count += 1;
            if count < 0 {
                newx = oldx.wrapping_add(self.cdx >> dshift);
                self.cdx = self.cdx.wrapping_add(self.cddx >> ddshift);
                self.cddx = self.cddx.wrapping_add(self.cdddx);

                newy = oldy.wrapping_add(self.cdy >> dshift);
                self.cdy = self.cdy.wrapping_add(self.cddy >> ddshift);
                self.cddy = self.cddy.wrapping_add(self.cdddy);
            } else {
                newx = self.clast_x;
                newy = self.clast_y;
            }

            if sort_y && newy < oldy {
                newy = oldy;
            }

            let mut new_snapped_y = snap_y(newy);
            if sort_y && self.clast_y < new_snapped_y {
                new_snapped_y = self.clast_y;
                count = 0;
            }

            let slope = if fixed_to_fdot6(new_snapped_y.wrapping_sub(self.snapped_y)) == 0 {
                MAX_S32
            } else {
                fdot6_div(
                    fixed_to_fdot6(newx.wrapping_sub(oldx)),
                    fixed_to_fdot6(new_snapped_y.wrapping_sub(self.snapped_y)),
                )
            };

            success = self.update_line(oldx, self.snapped_y, newx, new_snapped_y, slope);

            oldx = newx;
            oldy = newy;
            self.snapped_y = new_snapped_y;
            if !(count < 0 && !success) {
                break;
            }
        }

        self.cx = newx;
        self.cy = newy;
        self.curve_count = count as i8;
        success
    }

    fn keep_continuous_cubic(&mut self) {
        self.cx = self.x;
        self.snapped_y = self.y;
    }
}

fn fdot6_to_fixed_div2(value: FDot6) -> Fixed {
    left_shift(value, 16 - 6 - 1)
}

fn cheap_distance(mut dx: FDot6, mut dy: FDot6) -> FDot6 {
    dx = dx.wrapping_abs();
    dy = dy.wrapping_abs();
    if dx > dy {
        dx.wrapping_add(dy >> 1)
    } else {
        dy.wrapping_add(dx >> 1)
    }
}

fn diff_to_shift(dx: FDot6, dy: FDot6, shift_aa: i32) -> i32 {
    let mut dist = cheap_distance(dx, dy);
    dist = (dist.wrapping_add(1 << 4)) >> (3 + shift_aa);
    (32 - clz(dist as u32)) >> 1
}

fn cubic_delta_from_line(a: FDot6, b: FDot6, c: FDot6, d: FDot6) -> FDot6 {
    let one_third = (a
        .wrapping_mul(8)
        .wrapping_sub(b.wrapping_mul(15))
        .wrapping_add(6i32.wrapping_mul(c))
        .wrapping_add(d))
    .wrapping_mul(19)
        >> 9;
    let two_third = (a
        .wrapping_add(6i32.wrapping_mul(b))
        .wrapping_sub(c.wrapping_mul(15))
        .wrapping_add(d.wrapping_mul(8)))
    .wrapping_mul(19)
        >> 9;
    one_third.wrapping_abs().max(two_third.wrapping_abs())
}

// --- SkAnalyticEdgeBuilder ---------------------------------------------------

#[derive(PartialEq, Eq)]
enum Combine {
    No,
    Partial,
    Total,
}

fn combine_vertical(edge: &AEdge, last: &mut AEdge) -> Combine {
    let approximately_equal = |a: Fixed, b: Fixed| a.wrapping_sub(b).wrapping_abs() < 0x100;
    if last.edge_type != EdgeType::Line || last.dx != 0 || edge.x != last.x {
        return Combine::No;
    }
    if edge.winding == last.winding {
        if edge.lower_y == last.upper_y {
            last.upper_y = edge.upper_y;
            last.y = last.upper_y;
            return Combine::Partial;
        }
        if approximately_equal(edge.upper_y, last.lower_y) {
            last.lower_y = edge.lower_y;
            return Combine::Partial;
        }
        return Combine::No;
    }
    if approximately_equal(edge.upper_y, last.upper_y) {
        if approximately_equal(edge.lower_y, last.lower_y) {
            return Combine::Total;
        }
        if edge.lower_y < last.lower_y {
            last.upper_y = edge.lower_y;
            last.y = last.upper_y;
            return Combine::Partial;
        }
        last.upper_y = last.lower_y;
        last.y = last.upper_y;
        last.lower_y = edge.lower_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    if approximately_equal(edge.lower_y, last.lower_y) {
        if edge.upper_y > last.upper_y {
            last.lower_y = edge.upper_y;
            return Combine::Partial;
        }
        last.lower_y = last.upper_y;
        last.upper_y = edge.upper_y;
        last.y = last.upper_y;
        last.winding = edge.winding;
        return Combine::Partial;
    }
    Combine::No
}

struct EdgeBuilder {
    edges: Vec<AEdge>,
}

impl EdgeBuilder {
    fn add_line(&mut self, pts: &[Point; 2]) {
        let mut edge = AEdge::default();
        if edge.set_line(pts[0], pts[1]) {
            let is_vertical = edge.dx == 0 && edge.edge_type == EdgeType::Line;
            let combine = if is_vertical && let Some(last) = self.edges.last_mut() {
                combine_vertical(&edge, last)
            } else {
                Combine::No
            };
            match combine {
                Combine::Total => {
                    self.edges.pop();
                }
                Combine::Partial => {}
                Combine::No => self.edges.push(edge),
            }
        }
    }

    fn add_quad(&mut self, pts: &[Point; 3]) {
        let mut edge = AEdge::default();
        if edge.set_quadratic(pts) {
            self.edges.push(edge);
        }
    }

    fn add_cubic(&mut self, pts: &[Point; 4]) {
        let mut edge = AEdge::default();
        if edge.set_cubic(pts, true) {
            self.edges.push(edge);
        }
    }

    /// `SkEdgeBuilder::buildEdges`, clipping to `clip` when the path isn't
    /// inside it.
    fn build(path: &Path, clip: Option<crate::geom::Rect>) -> Vec<AEdge> {
        let mut b = EdgeBuilder { edges: Vec::new() };
        let can_cull_to_the_right = !path.is_convex();
        let line_only = path.segment_mask() == LINE_SEGMENT_MASK;
        if let Some(clip) = clip {
            if line_only {
                // buildPoly with SkLineClipper::ClipLine
                for e in path.edges() {
                    if let PathEdge::Line(p) = e {
                        let lines = crate::clip::clip_line(&p, &clip, can_cull_to_the_right);
                        for w in lines.windows(2) {
                            b.add_line(&[w[0], w[1]]);
                        }
                    }
                }
            } else {
                let mut clipper = crate::clip::EdgeClipper {
                    can_cull_to_the_right,
                    out: Vec::new(),
                };
                for e in path.edges() {
                    match e {
                        PathEdge::Line(p) => clipper.clip_line(p[0], p[1], &clip),
                        PathEdge::Quad(p) => clipper.clip_quad(&p, &clip),
                        PathEdge::Conic(p, w) => {
                            let q = geom::Conic::new(p, w).to_quads(0.25);
                            for i in 0..(q.len() - 1) / 2 {
                                clipper.clip_quad(&[q[2 * i], q[2 * i + 1], q[2 * i + 2]], &clip);
                            }
                        }
                        PathEdge::Cubic(p) => clipper.clip_cubic(&p, &clip),
                    }
                }
                for seg in clipper.out {
                    match seg {
                        crate::clip::Seg::Line(p) => b.add_line(&p),
                        crate::clip::Seg::Quad(p) => b.add_quad(&p),
                        crate::clip::Seg::Cubic(p) => b.add_cubic(&p),
                    }
                }
            }
            return b.edges;
        }
        let handle_quad = |b: &mut EdgeBuilder, pts: &[Point; 3]| {
            let mut mono = [Point::default(); 5];
            let n = geom::chop_quad_at_y_extrema(pts, &mut mono);
            for i in 0..=n {
                b.add_quad(&[mono[i * 2], mono[i * 2 + 1], mono[i * 2 + 2]]);
            }
        };
        for e in path.edges() {
            match e {
                PathEdge::Line(p) => b.add_line(&p),
                PathEdge::Quad(p) if !line_only => handle_quad(&mut b, &p),
                PathEdge::Conic(p, w) if !line_only => {
                    let q = geom::Conic::new(p, w).to_quads(0.25);
                    for i in 0..(q.len() - 1) / 2 {
                        handle_quad(&mut b, &[q[2 * i], q[2 * i + 1], q[2 * i + 2]]);
                    }
                }
                PathEdge::Cubic(p) if !line_only => {
                    let mut mono = [Point::default(); 10];
                    let n = geom::chop_cubic_at_y_extrema(&p, &mut mono);
                    for i in 0..=n {
                        b.add_cubic(&[
                            mono[i * 3],
                            mono[i * 3 + 1],
                            mono[i * 3 + 2],
                            mono[i * 3 + 3],
                        ]);
                    }
                }
                _ => {}
            }
        }
        b.edges
    }
}

// --- the scan converter ----------------------------------------------------

#[inline]
fn catch_overflow(alpha: i32) -> u8 {
    (alpha - (alpha >> 8)) as u8
}

#[inline]
fn add_alpha(alpha: &mut u8, delta: u8) {
    *alpha = catch_overflow(*alpha as i32 + delta as i32);
}

#[inline]
fn safely_add_alpha(alpha: &mut u8, delta: u8) {
    *alpha = (0xFF).min(*alpha as i32 + delta as i32) as u8;
}

/// The additive blitters: `MaskAdditiveBlitter` (small paths: a mask
/// blitted once at the end) and `RunBasedAdditiveBlitter` /
/// `SafeRLEAdditiveBlitter` (a row of alphas, flushed to the real blitter
/// whenever the walk moves to another row).
enum Additive<'a> {
    Mask {
        real: &'a mut dyn Blitter,
        mask: Vec<u8>,
        bounds: IRect,
        row_bytes: i32,
        clip_rect: IRect,
    },
    Rle {
        real: &'a mut dyn Blitter,
        safe: bool,
        curr_y: i32,
        width: i32,
        left: i32,
        top: i32,
        row: Vec<u8>,
        dirty: bool,
    },
}

impl<'a> Additive<'a> {
    fn new_mask(real: &'a mut dyn Blitter, ir: IRect, clip: IRect) -> Self {
        let mut clip_rect = ir;
        if !clip_rect.intersect(&clip) {
            clip_rect = IRect::default();
        }
        // +2: one byte of slack at either end, as Skia's storage has.
        let row_bytes = ir.width();
        let mask = vec![0u8; (ir.height() * row_bytes + 2) as usize];
        Additive::Mask {
            real,
            mask,
            bounds: ir,
            row_bytes,
            clip_rect,
        }
    }

    fn new_rle(real: &'a mut dyn Blitter, ir: IRect, clip: IRect, safe: bool) -> Self {
        let mut sect = ir;
        if !sect.intersect(&clip) {
            sect = IRect::default();
        }
        let width = sect.width();
        Additive::Rle {
            real,
            safe,
            curr_y: sect.top - 1,
            width,
            left: sect.left,
            top: sect.top,
            row: vec![0u8; width.max(0) as usize],
            dirty: false,
        }
    }

    fn is_mask(&self) -> bool {
        matches!(self, Additive::Mask { .. })
    }

    /// Index of (x, y) in the mask (with the 1-byte front slack).
    fn mask_index(&self, x: i32, y: i32) -> usize {
        match self {
            Additive::Mask {
                bounds, row_bytes, ..
            } => (1 + (y - bounds.top) * row_bytes + (x - bounds.left)) as usize,
            _ => unreachable!(),
        }
    }

    fn mask_at(&mut self, x: i32, y: i32) -> &mut u8 {
        let i = self.mask_index(x, y);
        match self {
            Additive::Mask { mask, .. } => &mut mask[i],
            _ => unreachable!(),
        }
    }

    fn get_width(&self) -> i32 {
        match self {
            Additive::Mask { clip_rect, .. } => clip_rect.width(),
            Additive::Rle { width, .. } => *width,
        }
    }

    // RunBasedAdditiveBlitter internals
    fn flush(&mut self) {
        if let Additive::Rle {
            real,
            curr_y,
            top,
            row,
            dirty,
            left,
            ..
        } = self
            && *curr_y >= *top
        {
            if *dirty {
                for a in row.iter_mut() {
                    *a = if *a > 247 {
                        0xFF
                    } else if *a < 8 {
                        0
                    } else {
                        *a
                    };
                }
                if row.iter().any(|a| *a != 0) {
                    let mut runs = vec![1i16; row.len() + 1];
                    runs[row.len()] = 0;
                    real.blit_anti_h(*left, *curr_y, row, &runs);
                }
                row.fill(0);
                *dirty = false;
            }
            *curr_y = *top - 1;
        }
    }

    fn check_y(&mut self, y: i32) {
        let cy = match self {
            Additive::Rle { curr_y, .. } => *curr_y,
            _ => return,
        };
        if y != cy {
            self.flush();
            if let Additive::Rle { curr_y, .. } = self {
                *curr_y = y;
            }
        }
    }

    fn flush_if_y_changed(&mut self, y: Fixed, next_y: Fixed) {
        if self.is_mask() {
            return;
        }
        if fixed_floor_to_int(y) != fixed_floor_to_int(next_y) {
            self.flush();
        }
    }

    /// `blitAntiH(x, y, antialias[], len)` of the RLE blitters.
    fn blit_anti_h_row(&mut self, x: i32, y: i32, antialias: &[u8]) {
        if self.is_mask() {
            panic!("Don't use this; directly add alphas to the mask.");
        }
        self.check_y(y);
        if let Additive::Rle {
            safe,
            width,
            left,
            row,
            dirty,
            ..
        } = self
        {
            let mut x = x - *left;
            let mut aa = antialias;
            let mut len = aa.len() as i32;
            if x < 0 {
                len += x;
                aa = &aa[(-x) as usize..];
                x = 0;
            }
            len = len.min(*width - x);
            for i in 0..len.max(0) {
                let a = &mut row[(x + i) as usize];
                if *safe {
                    safely_add_alpha(a, aa[i as usize]);
                } else {
                    add_alpha(a, aa[i as usize]);
                }
            }
            *dirty = true;
        }
    }

    /// `blitAntiH(x, y, alpha)`.
    fn blit_anti_h1(&mut self, x: i32, y: i32, alpha: u8) {
        match self {
            Additive::Mask { .. } => {
                add_alpha(self.mask_at(x, y), alpha);
            }
            Additive::Rle { .. } => {
                self.check_y(y);
                if let Additive::Rle {
                    safe,
                    width,
                    left,
                    row,
                    dirty,
                    ..
                } = self
                {
                    let x = x - *left;
                    if x >= 0 && x + 1 <= *width {
                        let a = &mut row[x as usize];
                        if *safe {
                            safely_add_alpha(a, alpha);
                        } else {
                            add_alpha(a, alpha);
                        }
                        *dirty = true;
                    }
                }
            }
        }
    }

    /// `blitAntiH(x, y, width, alpha)`.
    fn blit_anti_hn(&mut self, x: i32, y: i32, width: i32, alpha: u8) {
        match self {
            Additive::Mask { .. } => {
                for i in 0..width {
                    add_alpha(self.mask_at(x + i, y), alpha);
                }
            }
            Additive::Rle { .. } => {
                self.check_y(y);
                if let Additive::Rle {
                    safe,
                    width: w,
                    left,
                    row,
                    dirty,
                    ..
                } = self
                {
                    let x = x - *left;
                    if x >= 0 && x + width <= *w {
                        for i in 0..width {
                            let a = &mut row[(x + i) as usize];
                            if *safe {
                                safely_add_alpha(a, alpha);
                            } else {
                                add_alpha(a, alpha);
                            }
                        }
                        *dirty = true;
                    }
                }
            }
        }
    }

    // The "real blitter" calls: for the mask blitter, they set alphas.
    fn real_blit_v(&mut self, x: i32, y: i32, height: i32, alpha: u8) {
        match self {
            Additive::Mask { .. } => {
                if alpha == 0 {
                    return;
                }
                for i in 0..height {
                    *self.mask_at(x, y + i) = alpha;
                }
            }
            Additive::Rle { real, .. } => real.blit_v(x, y, height, alpha),
        }
    }

    fn real_blit_rect(&mut self, x: i32, y: i32, width: i32, height: i32) {
        match self {
            Additive::Mask { .. } => {
                for j in 0..height {
                    for i in 0..width {
                        *self.mask_at(x + i, y + j) = 0xFF;
                    }
                }
            }
            Additive::Rle { real, .. } => real.blit_rect(x, y, width, height),
        }
    }

    fn real_blit_anti_rect(&mut self, x: i32, y: i32, width: i32, height: i32, la: u8, ra: u8) {
        match self {
            Additive::Mask { .. } => {
                self.real_blit_v(x, y, height, la);
                self.real_blit_v(x + 1 + width, y, height, ra);
                self.real_blit_rect(x + 1, y, width, height);
            }
            Additive::Rle { real, .. } => real.blit_anti_rect(x, y, width, height, la, ra),
        }
    }

    fn real_blit_h(&mut self, x: i32, y: i32, width: i32) {
        match self {
            Additive::Rle { real, .. } => real.blit_h(x, y, width),
            Additive::Mask { .. } => unreachable!(),
        }
    }

    fn real_blit_anti_h2(&mut self, x: i32, y: i32, a0: u8, a1: u8) {
        match self {
            Additive::Rle { real, .. } => real.blit_anti_h2(x, y, a0, a1),
            Additive::Mask { .. } => unreachable!(),
        }
    }

    fn real_blit_anti_h_runs(&mut self, x: i32, y: i32, aa: &[u8], runs: &[i16]) {
        match self {
            Additive::Rle { real, .. } => real.blit_anti_h(x, y, aa, runs),
            Additive::Mask { .. } => unreachable!(),
        }
    }

    fn finish(mut self) {
        match &mut self {
            Additive::Mask {
                real,
                mask,
                bounds,
                row_bytes,
                clip_rect,
            } => {
                if !clip_rect.is_empty() {
                    real.blit_mask(&mask[1..], *bounds, *row_bytes, *clip_rect);
                }
            }
            Additive::Rle { .. } => self.flush(),
        }
    }
}

fn trapezoid_to_alpha(l1: Fixed, l2: Fixed) -> u8 {
    let area = l1.wrapping_add(l2) / 2;
    (area >> 8) as u8
}

fn partial_triangle_to_alpha(a: Fixed, b: Fixed) -> u8 {
    let area = (a >> 11).wrapping_mul(a >> 11).wrapping_mul(b >> 11);
    ((area >> 8) & 0xFF) as u8
}

fn get_partial_alpha_h(alpha: u8, partial_height: Fixed) -> u8 {
    fixed_round_to_int((alpha as i32).wrapping_mul(partial_height)) as u8
}

fn get_partial_alpha(alpha: u8, full_alpha: u8) -> u8 {
    ((alpha as i32 * full_alpha as i32) >> 8) as u8
}

fn fixed_to_alpha(f: Fixed) -> u8 {
    get_partial_alpha_h(0xFF, f)
}

fn approximate_intersection(mut l1: Fixed, mut r1: Fixed, mut l2: Fixed, mut r2: Fixed) -> Fixed {
    if l1 > r1 {
        std::mem::swap(&mut l1, &mut r1);
    }
    if l2 > r2 {
        std::mem::swap(&mut l2, &mut r2);
    }
    (l1.max(l2).wrapping_add(r1.min(r2))) / 2
}

fn compute_alpha_above_line(alphas: &mut [u8], l: Fixed, r: Fixed, dy: Fixed, full_alpha: u8) {
    let rr = fixed_ceil_to_int(r);
    if rr == 0 {
        return;
    } else if rr == 1 {
        alphas[0] = get_partial_alpha(
            (((rr << 17).wrapping_sub(l).wrapping_sub(r)) >> 9) as u8,
            full_alpha,
        );
    } else {
        let first = FIXED1 - l;
        let last = r - ((rr - 1) << 16);
        let first_h = fixed_mul(first, dy);
        alphas[0] = (fixed_mul(first, first_h) >> 9) as u8;
        let mut alpha16 = first_h.wrapping_add(dy >> 1);
        for a in alphas.iter_mut().take((rr - 1) as usize).skip(1) {
            *a = (alpha16 >> 8) as u8;
            alpha16 = alpha16.wrapping_add(dy);
        }
        alphas[(rr - 1) as usize] = full_alpha.wrapping_sub(partial_triangle_to_alpha(last, dy));
    }
}

fn compute_alpha_below_line(alphas: &mut [u8], l: Fixed, r: Fixed, dy: Fixed, full_alpha: u8) {
    let rr = fixed_ceil_to_int(r);
    if rr == 0 {
        return;
    } else if rr == 1 {
        alphas[0] = get_partial_alpha(trapezoid_to_alpha(l, r), full_alpha);
    } else {
        let first = FIXED1 - l;
        let last = r - ((rr - 1) << 16);
        let last_h = fixed_mul(last, dy);
        alphas[(rr - 1) as usize] = (fixed_mul(last, last_h) >> 9) as u8;
        let mut alpha16 = last_h.wrapping_add(dy >> 1);
        let mut i = rr - 2;
        while i > 0 {
            alphas[i as usize] = ((alpha16 >> 8) & 0xFF) as u8;
            alpha16 = alpha16.wrapping_add(dy);
            i -= 1;
        }
        alphas[0] = full_alpha.wrapping_sub(partial_triangle_to_alpha(first, dy));
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_single_alpha(
    blitter: &mut Additive,
    y: i32,
    x: i32,
    alpha: u8,
    full_alpha: u8,
    is_using_mask: bool,
    no_real_blitter: bool,
    need_safe_check: bool,
) {
    if is_using_mask {
        let p = blitter.mask_at(x, y);
        if full_alpha == 0xFF && !no_real_blitter {
            *p = alpha;
        } else if need_safe_check {
            safely_add_alpha(p, get_partial_alpha(alpha, full_alpha));
        } else {
            add_alpha(p, get_partial_alpha(alpha, full_alpha));
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter.real_blit_v(x, y, 1, alpha);
    } else {
        blitter.blit_anti_h1(x, y, get_partial_alpha(alpha, full_alpha));
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_two_alphas(
    blitter: &mut Additive,
    y: i32,
    x: i32,
    a1: u8,
    a2: u8,
    full_alpha: u8,
    is_using_mask: bool,
    no_real_blitter: bool,
    need_safe_check: bool,
) {
    if is_using_mask {
        if need_safe_check {
            safely_add_alpha(blitter.mask_at(x, y), a1);
            safely_add_alpha(blitter.mask_at(x + 1, y), a2);
        } else {
            add_alpha(blitter.mask_at(x, y), a1);
            add_alpha(blitter.mask_at(x + 1, y), a2);
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter.real_blit_anti_h2(x, y, a1, a2);
    } else {
        blitter.blit_anti_h1(x, y, a1);
        blitter.blit_anti_h1(x + 1, y, a2);
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_full_alpha(
    blitter: &mut Additive,
    y: i32,
    x: i32,
    len: i32,
    full_alpha: u8,
    is_using_mask: bool,
    no_real_blitter: bool,
    need_safe_check: bool,
) {
    if is_using_mask {
        for i in 0..len {
            let p = blitter.mask_at(x + i, y);
            if need_safe_check {
                safely_add_alpha(p, full_alpha);
            } else {
                add_alpha(p, full_alpha);
            }
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter.real_blit_h(x, y, len);
    } else {
        blitter.blit_anti_hn(x, y, len, full_alpha);
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_aaa_trapezoid_row(
    blitter: &mut Additive,
    y: i32,
    ul: Fixed,
    ur: Fixed,
    ll: Fixed,
    lr: Fixed,
    l_dy: Fixed,
    r_dy: Fixed,
    full_alpha: u8,
    is_using_mask: bool,
    no_real_blitter: bool,
    need_safe_check: bool,
) {
    let l = fixed_floor_to_int(ul);
    let r = fixed_ceil_to_int(lr);
    let len = r - l;

    if len == 1 {
        let alpha = trapezoid_to_alpha(ur.wrapping_sub(ul), lr.wrapping_sub(ll));
        blit_single_alpha(
            blitter,
            y,
            l,
            alpha,
            full_alpha,
            is_using_mask,
            no_real_blitter,
            need_safe_check,
        );
        return;
    }
    if len <= 0 {
        return;
    }

    let mut alphas = vec![full_alpha; (len + 1) as usize];
    let mut temp_alphas = vec![0u8; (len + 1) as usize];
    let runs = {
        let mut r = vec![1i16; (len + 1) as usize];
        r[len as usize] = 0;
        r
    };

    let u_l = fixed_floor_to_int(ul);
    let l_l = fixed_ceil_to_int(ll);
    if u_l + 2 == l_l {
        let first = int_to_fixed(u_l) + FIXED1 - ul;
        let second = ll - ul - first;
        let a1 = full_alpha.wrapping_sub(partial_triangle_to_alpha(first, l_dy));
        let a2 = partial_triangle_to_alpha(second, l_dy);
        alphas[0] = if alphas[0] > a1 { alphas[0] - a1 } else { 0 };
        alphas[1] = if alphas[1] > a2 { alphas[1] - a2 } else { 0 };
    } else {
        let off = (u_l - l) as usize;
        compute_alpha_below_line(
            &mut temp_alphas[off..],
            ul - int_to_fixed(u_l),
            ll - int_to_fixed(u_l),
            l_dy,
            full_alpha,
        );
        for i in u_l..l_l {
            let k = (i - l) as usize;
            if alphas[k] > temp_alphas[k] {
                alphas[k] -= temp_alphas[k];
            } else {
                alphas[k] = 0;
            }
        }
    }

    let u_r = fixed_floor_to_int(ur);
    let l_r = fixed_ceil_to_int(lr);
    if u_r + 2 == l_r {
        let first = int_to_fixed(u_r) + FIXED1 - ur;
        let second = lr - ur - first;
        let a1 = partial_triangle_to_alpha(first, r_dy);
        let a2 = full_alpha.wrapping_sub(partial_triangle_to_alpha(second, r_dy));
        let n = len as usize;
        alphas[n - 2] = if alphas[n - 2] > a1 {
            alphas[n - 2] - a1
        } else {
            0
        };
        alphas[n - 1] = if alphas[n - 1] > a2 {
            alphas[n - 1] - a2
        } else {
            0
        };
    } else {
        let off = (u_r - l) as usize;
        compute_alpha_above_line(
            &mut temp_alphas[off..],
            ur - int_to_fixed(u_r),
            lr - int_to_fixed(u_r),
            r_dy,
            full_alpha,
        );
        for i in u_r..l_r {
            let k = (i - l) as usize;
            if alphas[k] > temp_alphas[k] {
                alphas[k] -= temp_alphas[k];
            } else {
                alphas[k] = 0;
            }
        }
    }

    if is_using_mask {
        for i in 0..len {
            let p = blitter.mask_at(l + i, y);
            if need_safe_check {
                safely_add_alpha(p, alphas[i as usize]);
            } else {
                add_alpha(p, alphas[i as usize]);
            }
        }
    } else if full_alpha == 0xFF && !no_real_blitter {
        blitter.real_blit_anti_h_runs(l, y, &alphas[..len as usize], &runs);
    } else {
        blitter.blit_anti_h_row(l, y, &alphas[..len as usize]);
    }
}

#[allow(clippy::too_many_arguments)]
fn blit_trapezoid_row(
    blitter: &mut Additive,
    y: i32,
    mut ul: Fixed,
    mut ur: Fixed,
    mut ll: Fixed,
    mut lr: Fixed,
    l_dy: Fixed,
    r_dy: Fixed,
    full_alpha: u8,
    is_using_mask: bool,
    no_real_blitter: bool,
    need_safe_check: bool,
) {
    if ul > ur {
        return;
    }
    if ll > lr {
        let v = approximate_intersection(ul, ll, ur, lr);
        ll = v;
        lr = v;
    }
    if ul == ur && ll == lr {
        return;
    }
    if ul > ll {
        std::mem::swap(&mut ul, &mut ll);
    }
    if ur > lr {
        std::mem::swap(&mut ur, &mut lr);
    }

    let join_left = fixed_ceil_to_fixed(ll);
    let join_rite = fixed_floor_to_fixed(ur);
    if join_left <= join_rite {
        if ul < join_left {
            let len = fixed_ceil_to_int(join_left - ul);
            if len == 1 {
                let alpha = trapezoid_to_alpha(join_left - ul, join_left - ll);
                blit_single_alpha(
                    blitter,
                    y,
                    ul >> 16,
                    alpha,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter,
                    need_safe_check,
                );
            } else if len == 2 {
                let first = join_left - FIXED1 - ul;
                let second = ll - ul - first;
                let a1 = partial_triangle_to_alpha(first, l_dy);
                let a2 = full_alpha.wrapping_sub(partial_triangle_to_alpha(second, l_dy));
                blit_two_alphas(
                    blitter,
                    y,
                    ul >> 16,
                    a1,
                    a2,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter,
                    need_safe_check,
                );
            } else {
                blit_aaa_trapezoid_row(
                    blitter,
                    y,
                    ul,
                    join_left,
                    ll,
                    join_left,
                    l_dy,
                    MAX_S32,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter,
                    need_safe_check,
                );
            }
        }
        if join_left < join_rite {
            blit_full_alpha(
                blitter,
                y,
                fixed_floor_to_int(join_left),
                fixed_floor_to_int(join_rite - join_left),
                full_alpha,
                is_using_mask,
                no_real_blitter,
                need_safe_check,
            );
        }
        if lr > join_rite {
            let len = fixed_ceil_to_int(lr - join_rite);
            if len == 1 {
                let alpha = trapezoid_to_alpha(ur - join_rite, lr - join_rite);
                blit_single_alpha(
                    blitter,
                    y,
                    join_rite >> 16,
                    alpha,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter,
                    need_safe_check,
                );
            } else if len == 2 {
                let first = join_rite + FIXED1 - ur;
                let second = lr - ur - first;
                let a1 = full_alpha.wrapping_sub(partial_triangle_to_alpha(first, r_dy));
                let a2 = partial_triangle_to_alpha(second, r_dy);
                blit_two_alphas(
                    blitter,
                    y,
                    join_rite >> 16,
                    a1,
                    a2,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter,
                    need_safe_check,
                );
            } else {
                blit_aaa_trapezoid_row(
                    blitter,
                    y,
                    join_rite,
                    ur,
                    join_rite,
                    lr,
                    MAX_S32,
                    r_dy,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter,
                    need_safe_check,
                );
            }
        }
    } else {
        blit_aaa_trapezoid_row(
            blitter,
            y,
            ul,
            ur,
            ll,
            lr,
            l_dy,
            r_dy,
            full_alpha,
            is_using_mask,
            no_real_blitter,
            need_safe_check,
        );
    }
}

/// `operator<` on edges.
fn edge_less(a: &AEdge, b: &AEdge) -> bool {
    let (mut va, mut vb) = (a.upper_y, b.upper_y);
    if va == vb {
        va = a.x;
        vb = b.x;
    }
    if va == vb {
        va = a.dx;
        vb = b.dx;
    }
    va < vb
}

/// `SkTQSort` (introsort, insertion sort under 33 items) over indices,
/// with `less` as the C++ `operator<`.
pub(crate) fn sk_tq_sort(list: &mut [usize], less: &dyn Fn(usize, usize) -> bool) {
    let n = list.len();
    if n <= 1 {
        return;
    }
    let depth = 2 * (32 - clz((n - 1) as u32 - 1));
    intro_sort(depth, list, less);
}

fn insertion_sort(list: &mut [usize], less: &dyn Fn(usize, usize) -> bool) {
    let count = list.len();
    for next in 1..count {
        if !less(list[next], list[next - 1]) {
            continue;
        }
        let insert = list[next];
        let mut hole = next;
        loop {
            list[hole] = list[hole - 1];
            hole -= 1;
            if !(0 < hole && less(insert, list[hole - 1])) {
                break;
            }
        }
        list[hole] = insert;
    }
}

fn heap_sift_down(
    array: &mut [usize],
    mut root: usize,
    bottom: usize,
    less: &dyn Fn(usize, usize) -> bool,
) {
    let x = array[root - 1];
    let mut child = root << 1;
    while child <= bottom {
        if child < bottom && less(array[child - 1], array[child]) {
            child += 1;
        }
        if less(x, array[child - 1]) {
            array[root - 1] = array[child - 1];
            root = child;
            child = root << 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

fn heap_sift_up(
    array: &mut [usize],
    mut root: usize,
    bottom: usize,
    less: &dyn Fn(usize, usize) -> bool,
) {
    let x = array[root - 1];
    let start = root;
    let mut j = root << 1;
    while j <= bottom {
        if j < bottom && less(array[j - 1], array[j]) {
            j += 1;
        }
        array[root - 1] = array[j - 1];
        root = j;
        j = root << 1;
    }
    j = root >> 1;
    while j >= start {
        if less(array[j - 1], x) {
            array[root - 1] = array[j - 1];
            root = j;
            j = root >> 1;
        } else {
            break;
        }
    }
    array[root - 1] = x;
}

fn heap_sort(array: &mut [usize], less: &dyn Fn(usize, usize) -> bool) {
    let count = array.len();
    let mut i = count >> 1;
    while i > 0 {
        heap_sift_down(array, i, count, less);
        i -= 1;
    }
    let mut i = count - 1;
    while i > 0 {
        array.swap(0, i);
        heap_sift_up(array, 1, i, less);
        i -= 1;
    }
}

fn intro_sort(mut depth: i32, mut list: &mut [usize], less: &dyn Fn(usize, usize) -> bool) {
    loop {
        let count = list.len();
        if count <= 32 {
            insertion_sort(list, less);
            return;
        }
        if depth == 0 {
            heap_sort(list, less);
            return;
        }
        depth -= 1;
        let middle = (count - 1) >> 1;
        let right = count - 1;
        let pivot_value = list[middle];
        list.swap(middle, right);
        let mut new_pivot = 0;
        for left in 0..right {
            if less(list[left], pivot_value) {
                list.swap(left, new_pivot);
                new_pivot += 1;
            }
        }
        list.swap(new_pivot, right);
        let pivot_count = new_pivot;
        let (l, r) = list.split_at_mut(pivot_count);
        intro_sort(depth, l, less);
        list = &mut r[1..];
    }
}

struct Walk<'e> {
    e: &'e mut Vec<AEdge>,
}

impl Walk<'_> {
    fn remove_edge(&mut self, edge: usize) {
        let (prev, next) = (self.e[edge].prev, self.e[edge].next);
        self.e[prev].next = next;
        self.e[next].prev = prev;
    }

    fn insert_edge_after(&mut self, edge: usize, after: usize) {
        let after_next = self.e[after].next;
        self.e[edge].prev = after;
        self.e[edge].next = after_next;
        self.e[after_next].prev = edge;
        self.e[after].next = edge;
    }

    fn backward_insert_edge_based_on_x(&mut self, edge: usize) {
        let x = self.e[edge].x;
        let mut prev = self.e[edge].prev;
        while self.e[prev].prev != NONE && self.e[prev].x > x {
            prev = self.e[prev].prev;
        }
        if self.e[prev].next != edge {
            self.remove_edge(edge);
            self.insert_edge_after(edge, prev);
        }
    }

    fn backward_insert_start(&self, mut prev: usize, x: Fixed) -> usize {
        while self.e[prev].prev != NONE && self.e[prev].x > x {
            prev = self.e[prev].prev;
        }
        prev
    }

    fn is_smooth_enough1(&self, this: usize, next: usize) -> bool {
        let t = &self.e[this];
        if t.curve_count < 0 {
            let ddshift = t.c_curve_shift as i32;
            return (t.cdx.wrapping_abs() >> 1) >= (t.cddx.wrapping_abs() >> ddshift)
                && (t.cdy.wrapping_abs() >> 1) >= (t.cddy.wrapping_abs() >> ddshift)
                && (t.cdy.wrapping_sub(t.cddy >> ddshift)) >> t.c_cubic_dshift >= FIXED1;
        } else if t.curve_count > 0 {
            return (t.qdx.wrapping_abs() >> 1) >= t.qddx.wrapping_abs()
                && (t.qdy.wrapping_abs() >> 1) >= t.qddy.wrapping_abs()
                && (t.qdy.wrapping_sub(t.qddy)) >> t.q_curve_shift >= FIXED1;
        }
        let n = &self.e[next];
        n.dx.wrapping_sub(t.dx).wrapping_abs() <= FIXED1
            && n.lower_y.wrapping_sub(n.upper_y) >= FIXED1
    }

    fn is_smooth_enough(
        &self,
        left_e: usize,
        rite_e: usize,
        mut curr_e: usize,
        stop_y: i32,
    ) -> bool {
        if self.e[curr_e].upper_y >= left_shift(stop_y, 16) {
            return false;
        }
        let (l, r) = (&self.e[left_e], &self.e[rite_e]);
        if l.lower_y.wrapping_add(FIXED1) < r.lower_y {
            return self.is_smooth_enough1(left_e, curr_e);
        } else if l.lower_y > r.lower_y.wrapping_add(FIXED1) {
            return self.is_smooth_enough1(rite_e, curr_e);
        }
        let mut next_curr_e = self.e[curr_e].next;
        if self.e[next_curr_e].upper_y >= stop_y << 16 {
            return false;
        }
        if self.e[next_curr_e].upper_x < self.e[curr_e].upper_x {
            std::mem::swap(&mut curr_e, &mut next_curr_e);
        }
        self.is_smooth_enough1(left_e, curr_e) && self.is_smooth_enough1(rite_e, next_curr_e)
    }
}

#[allow(clippy::too_many_arguments)]
fn aaa_walk_convex_edges(
    w: &mut Walk,
    head: usize,
    blitter: &mut Additive,
    _start_y: i32,
    stop_y: i32,
    left_bound: Fixed,
    rite_bound: Fixed,
    is_using_mask: bool,
) {
    let mut left_e = w.e[head].next;
    let mut rite_e = w.e[left_e].next;
    let mut curr_e = w.e[rite_e].next;

    let mut y = w.e[left_e].upper_y.max(w.e[rite_e].upper_y);

    loop {
        while w.e[left_e].lower_y <= y {
            if !w.e[left_e].update(y) {
                if fixed_floor_to_int(w.e[curr_e].upper_y) >= stop_y {
                    return;
                }
                left_e = curr_e;
                curr_e = w.e[curr_e].next;
            }
        }
        while w.e[rite_e].lower_y <= y {
            if !w.e[rite_e].update(y) {
                if fixed_floor_to_int(w.e[curr_e].upper_y) >= stop_y {
                    return;
                }
                rite_e = curr_e;
                curr_e = w.e[curr_e].next;
            }
        }

        if fixed_floor_to_int(y) >= stop_y {
            break;
        }

        w.e[left_e].go_y(y);
        w.e[rite_e].go_y(y);

        if w.e[left_e].x > w.e[rite_e].x
            || (w.e[left_e].x == w.e[rite_e].x && w.e[left_e].dx > w.e[rite_e].dx)
        {
            std::mem::swap(&mut left_e, &mut rite_e);
        }

        let mut local_bot_fixed = w.e[left_e].lower_y.min(w.e[rite_e].lower_y);
        if w.is_smooth_enough(left_e, rite_e, curr_e, stop_y) {
            local_bot_fixed = fixed_ceil_to_fixed(local_bot_fixed);
        }
        local_bot_fixed = local_bot_fixed.min(int_to_fixed(stop_y));

        let mut left = left_bound.max(w.e[left_e].x);
        let d_left = w.e[left_e].dx;
        let mut rite = rite_bound.min(w.e[rite_e].x);
        let d_rite = w.e[rite_e].dx;
        if 0 == (d_left | d_rite) {
            let full_left = fixed_ceil_to_int(left);
            let full_rite = fixed_floor_to_int(rite);
            let partial_left = int_to_fixed(full_left) - left;
            let partial_rite = rite - int_to_fixed(full_rite);
            let full_top = fixed_ceil_to_int(y);
            let full_bot = fixed_floor_to_int(local_bot_fixed);
            let mut partial_top = int_to_fixed(full_top) - y;
            let mut partial_bot = local_bot_fixed - int_to_fixed(full_bot);
            if full_top > full_bot {
                partial_top -= FIXED1 - partial_bot;
                partial_bot = 0;
            }

            if full_rite >= full_left {
                if partial_top > 0 {
                    if partial_left > 0 {
                        blitter.blit_anti_h1(
                            full_left - 1,
                            full_top - 1,
                            fixed_to_alpha(fixed_mul(partial_top, partial_left)),
                        );
                    }
                    blitter.blit_anti_hn(
                        full_left,
                        full_top - 1,
                        full_rite - full_left,
                        fixed_to_alpha(partial_top),
                    );
                    if partial_rite > 0 {
                        blitter.blit_anti_h1(
                            full_rite,
                            full_top - 1,
                            fixed_to_alpha(fixed_mul(partial_top, partial_rite)),
                        );
                    }
                    blitter.flush_if_y_changed(y, y + partial_top);
                }

                if full_bot > full_top
                    && (full_rite > full_left
                        || fixed_to_alpha(partial_left) > 0
                        || fixed_to_alpha(partial_rite) > 0)
                {
                    blitter.real_blit_anti_rect(
                        full_left - 1,
                        full_top,
                        full_rite - full_left,
                        full_bot - full_top,
                        fixed_to_alpha(partial_left),
                        fixed_to_alpha(partial_rite),
                    );
                }

                if partial_bot > 0 {
                    if partial_left > 0 {
                        blitter.blit_anti_h1(
                            full_left - 1,
                            full_bot,
                            fixed_to_alpha(fixed_mul(partial_bot, partial_left)),
                        );
                    }
                    blitter.blit_anti_hn(
                        full_left,
                        full_bot,
                        full_rite - full_left,
                        fixed_to_alpha(partial_bot),
                    );
                    if partial_rite > 0 {
                        blitter.blit_anti_h1(
                            full_rite,
                            full_bot,
                            fixed_to_alpha(fixed_mul(partial_bot, partial_rite)),
                        );
                    }
                }
            } else {
                let width = rite - left;
                if width > 0 {
                    if partial_top > 0 {
                        blitter.blit_anti_hn(
                            full_left - 1,
                            full_top - 1,
                            1,
                            fixed_to_alpha(fixed_mul(partial_top, width)),
                        );
                        blitter.flush_if_y_changed(y, y + partial_top);
                    }
                    if full_bot > full_top {
                        blitter.real_blit_v(
                            full_left - 1,
                            full_top,
                            full_bot - full_top,
                            fixed_to_alpha(width),
                        );
                    }
                    if partial_bot > 0 {
                        blitter.blit_anti_hn(
                            full_left - 1,
                            full_bot,
                            1,
                            fixed_to_alpha(fixed_mul(partial_bot, width)),
                        );
                    }
                }
            }

            y = local_bot_fixed;
        } else {
            const SNAP_DIGIT: Fixed = FIXED1 >> 4;
            const SNAP_HALF: Fixed = SNAP_DIGIT >> 1;
            const SNAP_MASK: Fixed = -1 ^ (SNAP_DIGIT - 1);
            left = left.wrapping_add(SNAP_HALF);
            rite = rite.wrapping_add(SNAP_HALF);

            let mut count = fixed_ceil_to_int(local_bot_fixed) - fixed_floor_to_int(y);

            if count > 1 {
                if (y as u32 & 0xFFFF_0000) as i32 != y {
                    count -= 1;
                    let next_y = fixed_ceil_to_fixed(y + 1);
                    let dy = next_y - y;
                    let next_left = left.wrapping_add(fixed_mul(d_left, dy));
                    let next_rite = rite.wrapping_add(fixed_mul(d_rite, dy));
                    let (ldy, rdy) = (w.e[left_e].dy, w.e[rite_e].dy);
                    blit_trapezoid_row(
                        blitter,
                        y >> 16,
                        left & SNAP_MASK,
                        rite & SNAP_MASK,
                        next_left & SNAP_MASK,
                        next_rite & SNAP_MASK,
                        ldy,
                        rdy,
                        get_partial_alpha_h(0xFF, dy),
                        is_using_mask,
                        false,
                        false,
                    );
                    blitter.flush_if_y_changed(y, next_y);
                    left = next_left;
                    rite = next_rite;
                    y = next_y;
                }

                while count > 1 {
                    count -= 1;
                    let next_y = y + FIXED1;
                    let next_left = left.wrapping_add(d_left);
                    let next_rite = rite.wrapping_add(d_rite);
                    let (ldy, rdy) = (w.e[left_e].dy, w.e[rite_e].dy);
                    blit_trapezoid_row(
                        blitter,
                        y >> 16,
                        left & SNAP_MASK,
                        rite & SNAP_MASK,
                        next_left & SNAP_MASK,
                        next_rite & SNAP_MASK,
                        ldy,
                        rdy,
                        0xFF,
                        is_using_mask,
                        false,
                        false,
                    );
                    blitter.flush_if_y_changed(y, next_y);
                    left = next_left;
                    rite = next_rite;
                    y = next_y;
                }
            }

            let dy = local_bot_fixed - y;
            let next_left = left
                .wrapping_add(fixed_mul(d_left, dy))
                .max(left_bound + SNAP_HALF);
            let next_rite = rite
                .wrapping_add(fixed_mul(d_rite, dy))
                .min(rite_bound + SNAP_HALF);
            let (ldy, rdy) = (w.e[left_e].dy, w.e[rite_e].dy);
            blit_trapezoid_row(
                blitter,
                y >> 16,
                left & SNAP_MASK,
                rite & SNAP_MASK,
                next_left & SNAP_MASK,
                next_rite & SNAP_MASK,
                ldy,
                rdy,
                get_partial_alpha_h(0xFF, dy),
                is_using_mask,
                false,
                false,
            );
            blitter.flush_if_y_changed(y, local_bot_fixed);
            left = next_left;
            rite = next_rite;
            y = local_bot_fixed;
            left -= SNAP_HALF;
            rite -= SNAP_HALF;
        }

        w.e[left_e].x = left;
        w.e[rite_e].x = rite;
        w.e[left_e].y = y;
        w.e[rite_e].y = y;
    }
}

fn update_next_next_y(y: Fixed, next_y: Fixed, next_next_y: &mut Fixed) {
    *next_next_y = if y > next_y && y < *next_next_y {
        y
    } else {
        *next_next_y
    };
}

fn check_intersection(w: &Walk, edge: usize, next_y: Fixed, next_next_y: &mut Fixed) {
    let e = &w.e[edge];
    let p = &w.e[e.prev];
    if p.prev != NONE && p.x.wrapping_add(p.dx) > e.x.wrapping_add(e.dx) {
        *next_next_y = next_y + (FIXED1 >> DEFAULT_ACCURACY);
    }
}

fn insert_new_edges(w: &mut Walk, mut new_edge: usize, y: Fixed, next_next_y: &mut Fixed) {
    if w.e[new_edge].upper_y > y {
        update_next_next_y(w.e[new_edge].upper_y, y, next_next_y);
        return;
    }
    let prev = w.e[new_edge].prev;
    if w.e[prev].x <= w.e[new_edge].x {
        while w.e[new_edge].upper_y <= y {
            check_intersection(w, new_edge, y, next_next_y);
            update_next_next_y(w.e[new_edge].lower_y, y, next_next_y);
            new_edge = w.e[new_edge].next;
        }
        update_next_next_y(w.e[new_edge].upper_y, y, next_next_y);
        return;
    }
    let mut start = w.backward_insert_start(prev, w.e[new_edge].x);
    loop {
        let next = w.e[new_edge].next;
        let mut skip = false;
        loop {
            if w.e[start].next == new_edge {
                skip = true;
                break;
            }
            let after = w.e[start].next;
            if w.e[after].x >= w.e[new_edge].x {
                break;
            }
            start = after;
        }
        if !skip {
            w.remove_edge(new_edge);
            w.insert_edge_after(new_edge, start);
        }
        check_intersection(w, new_edge, y, next_next_y);
        update_next_next_y(w.e[new_edge].lower_y, y, next_next_y);
        start = new_edge;
        new_edge = next;
        if w.e[new_edge].upper_y > y {
            break;
        }
    }
    update_next_next_y(w.e[new_edge].upper_y, y, next_next_y);
}

fn edges_too_close(w: &Walk, prev: usize, next: usize, lower_y: Fixed) -> bool {
    const SLACK: Fixed = FIXED1;
    next != NONE
        && prev != NONE
        && w.e[next].upper_y < lower_y
        && w.e[prev].x.wrapping_add(SLACK) >= w.e[next].x.wrapping_sub(w.e[next].dx.wrapping_abs())
}

fn edges_too_close_rite(prev_rite: i32, ul: Fixed, ll: Fixed) -> bool {
    prev_rite > fixed_floor_to_int(ul) || prev_rite > fixed_floor_to_int(ll)
}

#[allow(clippy::too_many_arguments)]
fn blit_saved_trapezoid(
    w: &mut Walk,
    left_e: usize,
    lower_y: Fixed,
    lower_left: Fixed,
    lower_rite: Fixed,
    blitter: &mut Additive,
    is_using_mask: bool,
    no_real_blitter: bool,
    left_clip: Fixed,
    right_clip: Fixed,
) {
    let rite_e = w.e[left_e].rite_e;
    let y = fixed_floor_to_int(w.e[left_e].saved_y);
    let full_alpha = fixed_to_alpha(lower_y - int_to_fixed(y))
        .wrapping_sub(fixed_to_alpha(w.e[left_e].saved_y - int_to_fixed(y)));
    let too_close = full_alpha == 0xFF
        && (edges_too_close(w, w.e[left_e].prev, left_e, lower_y)
            || edges_too_close(w, rite_e, w.e[rite_e].next, lower_y));
    blit_trapezoid_row(
        blitter,
        y,
        w.e[left_e].saved_x.max(left_clip),
        w.e[rite_e].saved_x.min(right_clip),
        lower_left.max(left_clip),
        lower_rite.min(right_clip),
        w.e[left_e].saved_dy,
        w.e[rite_e].saved_dy,
        full_alpha,
        is_using_mask,
        no_real_blitter || too_close,
        true,
    );
    w.e[left_e].rite_e = NONE;
}

#[allow(clippy::too_many_arguments)]
fn deferred_blit(
    w: &mut Walk,
    left_e: usize,
    rite_e: usize,
    left: Fixed,
    left_dy: Fixed,
    y: Fixed,
    next_y: Fixed,
    is_integral_next_y: bool,
    left_ends: bool,
    rite_ends: bool,
    blitter: &mut Additive,
    is_using_mask: bool,
    no_real_blitter: bool,
    left_clip: Fixed,
    right_clip: Fixed,
    y_shift: i32,
) {
    let lre = w.e[left_e].rite_e;
    if lre != NONE && lre != rite_e {
        let lr_x = w.e[lre].x;
        blit_saved_trapezoid(
            w,
            left_e,
            y,
            left,
            lr_x,
            blitter,
            is_using_mask,
            no_real_blitter,
            left_clip,
            right_clip,
        );
    }
    if w.e[left_e].rite_e == NONE {
        w.e[left_e].save_xy(left, y, left_dy);
        let (rx, rdy) = (w.e[rite_e].x, w.e[rite_e].dy);
        w.e[rite_e].save_xy(rx, y, rdy);
        w.e[left_e].rite_e = rite_e;
    }
    w.e[rite_e].go_y_shift(next_y, y_shift);
    if is_integral_next_y || left_ends || rite_ends {
        let (lx, rx) = (w.e[left_e].x, w.e[rite_e].x);
        blit_saved_trapezoid(
            w,
            left_e,
            next_y,
            lx,
            rx,
            blitter,
            is_using_mask,
            no_real_blitter,
            left_clip,
            right_clip,
        );
    }
}

#[allow(clippy::too_many_arguments)]
fn aaa_walk_edges(
    w: &mut Walk,
    prev_head: usize,
    next_tail: usize,
    fill_type: FillType,
    blitter: &mut Additive,
    start_y: i32,
    stop_y: i32,
    left_clip: Fixed,
    right_clip: Fixed,
    is_using_mask: bool,
    force_rle: bool,
    use_deferred: bool,
    skip_intersect: bool,
) {
    w.e[prev_head].x = left_clip;
    w.e[prev_head].upper_x = left_clip;
    w.e[next_tail].x = right_clip;
    w.e[next_tail].upper_x = right_clip;
    let first = w.e[prev_head].next;
    let mut y = w.e[first].upper_y.max(int_to_fixed(start_y));
    let mut next_next_y = MAX_S32;

    {
        let mut edge = w.e[prev_head].next;
        while w.e[edge].upper_y <= y {
            w.e[edge].go_y(y);
            update_next_next_y(w.e[edge].lower_y, y, &mut next_next_y);
            edge = w.e[edge].next;
        }
        update_next_next_y(w.e[edge].upper_y, y, &mut next_next_y);
    }

    let winding_mask: i32 = if fill_type == FillType::EvenOdd {
        1
    } else {
        -1
    };
    let is_inverse = false;

    loop {
        let mut wsum: i32 = 0;
        let mut in_interval = is_inverse;
        let mut prev_x = w.e[prev_head].x;
        let mut next_y = next_next_y.min(fixed_ceil_to_fixed(y + 1));
        let is_integral_next_y = (next_y & (FIXED1 - 1)) == 0;
        let mut curr_e = w.e[prev_head].next;
        let mut left_e = prev_head;
        let mut left = left_clip;
        let mut left_dy: Fixed = 0;
        let mut left_ends = false;
        let mut prev_rite = fixed_floor_to_int(left_clip);

        next_next_y = MAX_S32;

        let mut y_shift = 0;
        if (next_y - y) & (FIXED1 >> 2) != 0 {
            y_shift = 2;
            next_y = y + (FIXED1 >> 2);
        } else if (next_y - y) & (FIXED1 >> 1) != 0 {
            y_shift = 1;
        }

        let full_alpha = fixed_to_alpha(next_y - y);

        let no_real_blitter = force_rle;

        while w.e[curr_e].upper_y <= y {
            wsum += w.e[curr_e].winding as i32;
            let prev_in_interval = in_interval;
            in_interval = ((wsum & winding_mask) == 0) == is_inverse;

            let is_left = in_interval && !prev_in_interval;
            let is_rite = !in_interval && prev_in_interval;
            let curr_ends = w.e[curr_e].lower_y == next_y;

            if use_deferred {
                if w.e[curr_e].rite_e != NONE && !is_left {
                    let (cx, crx) = (w.e[curr_e].x, w.e[w.e[curr_e].rite_e].x);
                    blit_saved_trapezoid(
                        w,
                        curr_e,
                        y,
                        cx,
                        crx,
                        blitter,
                        is_using_mask,
                        no_real_blitter,
                        left_clip,
                        right_clip,
                    );
                }
                if w.e[left_e].rite_e == curr_e && !is_rite {
                    let cx = w.e[curr_e].x;
                    blit_saved_trapezoid(
                        w,
                        left_e,
                        y,
                        left,
                        cx,
                        blitter,
                        is_using_mask,
                        no_real_blitter,
                        left_clip,
                        right_clip,
                    );
                }
            }

            if is_rite {
                if use_deferred {
                    deferred_blit(
                        w,
                        left_e,
                        curr_e,
                        left,
                        left_dy,
                        y,
                        next_y,
                        is_integral_next_y,
                        left_ends,
                        curr_ends,
                        blitter,
                        is_using_mask,
                        no_real_blitter,
                        left_clip,
                        right_clip,
                        y_shift,
                    );
                } else {
                    let mut rite = w.e[curr_e].x;
                    w.e[curr_e].go_y_shift(next_y, y_shift);
                    let next_left = left_clip.max(w.e[left_e].x);
                    rite = right_clip.min(rite);
                    let next_rite = right_clip.min(w.e[curr_e].x);
                    let curr_next = w.e[curr_e].next;
                    let too_close = full_alpha == 0xFF
                        && (edges_too_close_rite(prev_rite, left, w.e[left_e].x)
                            || edges_too_close(w, curr_e, curr_next, next_y));
                    let cdy = w.e[curr_e].dy;
                    blit_trapezoid_row(
                        blitter,
                        y >> 16,
                        left,
                        rite,
                        next_left,
                        next_rite,
                        left_dy,
                        cdy,
                        full_alpha,
                        is_using_mask,
                        no_real_blitter || too_close,
                        true,
                    );
                    prev_rite = fixed_ceil_to_int(rite.max(w.e[curr_e].x));
                }
            } else {
                if is_left {
                    left = w.e[curr_e].x.max(left_clip);
                    left_dy = w.e[curr_e].dy;
                    left_e = curr_e;
                    left_ends = w.e[left_e].lower_y == next_y;
                }
                w.e[curr_e].go_y_shift(next_y, y_shift);
            }

            let next = w.e[curr_e].next;

            while w.e[curr_e].lower_y <= next_y {
                if w.e[curr_e].curve_count < 0 {
                    w.e[curr_e].keep_continuous_cubic();
                    if !w.e[curr_e].update_cubic(true) {
                        break;
                    }
                } else if w.e[curr_e].curve_count > 0 {
                    w.e[curr_e].keep_continuous_quad();
                    if !w.e[curr_e].update_quadratic() {
                        break;
                    }
                } else {
                    break;
                }
            }

            if w.e[curr_e].lower_y <= next_y {
                w.remove_edge(curr_e);
            } else {
                update_next_next_y(w.e[curr_e].lower_y, next_y, &mut next_next_y);
                let new_x = w.e[curr_e].x;
                if new_x < prev_x {
                    if w.e[left_e].rite_e == curr_e && use_deferred {
                        let (lx, cx) = (w.e[left_e].x, w.e[curr_e].x);
                        blit_saved_trapezoid(
                            w,
                            left_e,
                            next_y,
                            lx,
                            cx,
                            blitter,
                            is_using_mask,
                            no_real_blitter,
                            left_clip,
                            right_clip,
                        );
                    }
                    w.backward_insert_edge_based_on_x(curr_e);
                } else {
                    prev_x = new_x;
                }
                if !skip_intersect {
                    check_intersection(w, curr_e, next_y, &mut next_next_y);
                }
            }

            curr_e = next;
        }

        if in_interval {
            if use_deferred {
                deferred_blit(
                    w,
                    left_e,
                    next_tail,
                    left,
                    left_dy,
                    y,
                    next_y,
                    is_integral_next_y,
                    left_ends,
                    false,
                    blitter,
                    is_using_mask,
                    no_real_blitter,
                    left_clip,
                    right_clip,
                    y_shift,
                );
            } else {
                let too_close =
                    full_alpha == 0xFF && edges_too_close(w, w.e[left_e].prev, left_e, next_y);
                let lx = w.e[left_e].x;
                blit_trapezoid_row(
                    blitter,
                    y >> 16,
                    left,
                    right_clip,
                    left_clip.max(lx),
                    right_clip,
                    left_dy,
                    0,
                    full_alpha,
                    is_using_mask,
                    no_real_blitter || too_close,
                    true,
                );
            }
        }

        if force_rle {
            blitter.flush_if_y_changed(y, next_y);
        }

        y = next_y;
        if y >= int_to_fixed(stop_y) {
            break;
        }

        insert_new_edges(w, curr_e, y, &mut next_next_y);
    }
}

#[allow(clippy::too_many_arguments)]
fn aaa_fill_path(
    path: &Path,
    clip_rect: IRect,
    blitter: &mut Additive,
    start_y: i32,
    stop_y: i32,
    path_contained_in_clip: bool,
    is_using_mask: bool,
    force_rle: bool,
) {
    let mut edges = EdgeBuilder::build(
        path,
        if path_contained_in_clip {
            None
        } else {
            Some(clip_rect.to_rect())
        },
    );
    let count = edges.len();
    if count == 0 {
        return;
    }
    let rect = clip_rect;

    let mut list: Vec<usize> = (0..count).collect();
    sk_tq_sort(&mut list, &|a, b| edge_less(&edges[a], &edges[b]));

    // head and tail sentinels
    let head = edges.len();
    let tail = head + 1;
    edges.push(AEdge::default());
    edges.push(AEdge::default());
    for i in 1..count {
        edges[list[i - 1]].next = list[i];
        edges[list[i]].prev = list[i - 1];
    }
    let first = list[0];
    let last = list[count - 1];

    {
        let h = &mut edges[head];
        h.rite_e = NONE;
        h.prev = NONE;
        h.next = first;
        h.upper_y = MIN_S32;
        h.lower_y = MIN_S32;
        h.x = MIN_S32;
        h.dx = 0;
        h.dy = MAX_S32;
        h.upper_x = MIN_S32;
    }
    edges[first].prev = head;
    {
        let t = &mut edges[tail];
        t.rite_e = NONE;
        t.prev = last;
        t.next = NONE;
        t.upper_y = MAX_S32;
        t.lower_y = MAX_S32;
        t.x = MAX_S32;
        t.dx = 0;
        t.dy = MAX_S32;
        t.upper_x = MAX_S32;
    }
    edges[last].next = tail;

    let (mut start_y, mut stop_y) = (start_y, stop_y);
    if !path_contained_in_clip && start_y < clip_rect.top {
        start_y = clip_rect.top;
    }
    if !path_contained_in_clip && stop_y > clip_rect.bottom {
        stop_y = clip_rect.bottom;
    }

    let mut left_bound = int_to_fixed(rect.left);
    let mut right_bound = int_to_fixed(rect.right);
    if is_using_mask {
        let ir = path.bounds().round_out();
        left_bound = left_bound.max(int_to_fixed(ir.left));
        right_bound = right_bound.min(int_to_fixed(ir.right));
    }

    let mut w = Walk { e: &mut edges };
    if path.is_convex() && count >= 2 {
        aaa_walk_convex_edges(
            &mut w,
            head,
            blitter,
            start_y,
            stop_y,
            left_bound,
            right_bound,
            is_using_mask,
        );
    } else {
        let tail_prev_lower = w.e[w.e[tail].prev].lower_y;
        let head_next_upper = w.e[w.e[head].next].upper_y;
        let use_deferred = count as i32
            > (fixed_floor_to_int(tail_prev_lower.wrapping_sub(head_next_upper)) + 1) * 4;
        let skip_intersect = path.count_points() as i32 > (stop_y - start_y) * 2;
        aaa_walk_edges(
            &mut w,
            head,
            tail,
            path.fill_type,
            blitter,
            start_y,
            stop_y,
            left_bound,
            right_bound,
            is_using_mask,
            force_rle,
            use_deferred,
            skip_intersect,
        );
    }
}

/// `MaskAdditiveBlitter::CanHandleRect`.
fn can_handle_rect(bounds: &IRect) -> bool {
    const MAX_WIDTH: i32 = 32;
    const MAX_STORAGE: i64 = 1024;
    let width = bounds.width();
    if width > MAX_WIDTH {
        return false;
    }
    let rb = ((width + 3) & !3) as i64;
    let storage = rb * bounds.height() as i64;
    storage <= MAX_STORAGE
}

/// `SkScan::AAAFillPath`.
pub fn aaa_fill_path_entry(path: &Path, blitter: &mut dyn Blitter, ir: IRect, clip_bounds: IRect) {
    let contained_in_clip = clip_bounds.contains(&ir);
    if can_handle_rect(&ir) {
        if let Some(rect) = path.is_rect() {
            let mut r = rect;
            if !r.intersect(&clip_bounds.to_rect()) {
                return;
            }
            let bounds = r.round_out();
            if bounds.width() >= 3 {
                blitter.blit_fat_anti_rect(r);
                return;
            }
        }
        let mut add = Additive::new_mask(blitter, ir, clip_bounds);
        aaa_fill_path(
            path,
            clip_bounds,
            &mut add,
            ir.top,
            ir.bottom,
            contained_in_clip,
            true,
            false,
        );
        add.finish();
    } else if path.is_convex() {
        let mut add = Additive::new_rle(blitter, ir, clip_bounds, false);
        aaa_fill_path(
            path,
            clip_bounds,
            &mut add,
            ir.top,
            ir.bottom,
            contained_in_clip,
            false,
            false,
        );
        add.finish();
    } else {
        let mut add = Additive::new_rle(blitter, ir, clip_bounds, true);
        aaa_fill_path(
            path,
            clip_bounds,
            &mut add,
            ir.top,
            ir.bottom,
            contained_in_clip,
            false,
            false,
        );
        add.finish();
    }
    let _ = Additive::get_width;
}
