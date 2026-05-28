use core::ptr::{addr_of, addr_of_mut};

pub const FFT_SIZE: usize = 512; // 512-point FFT

/// A complex number represented in Q1.12 fixed-point format (Re + i * Im)
/// Real values are scaled by 2^12 = 4096. i16 allows range of [-8.0, 7.99]
#[derive(Clone, Copy, Default, Debug)]
pub struct FixedComplex {
    pub re: i16,
    pub im: i16,
}

/// Twiddle factor tables: Cosine and Sine values for all butterfly stages.
/// Stored in static mutable variables initialized once at boot.
static mut TWIDDLE_RE: [i16; FFT_SIZE / 2] = [0i16; FFT_SIZE / 2];
static mut TWIDDLE_IM: [i16; FFT_SIZE / 2] = [0i16; FFT_SIZE / 2];

// CORDIC lookup table: arctan(2^-j) in microradians (scaled by 1,000,000) for j = 0..11
const CORDIC_ANGLES: [u32; 12] = [
    785398, // atan(1.0) = 0.785398 rad
    463647, // atan(0.5) = 0.463647 rad
    244978, // ...
    124354, 62418, 31239, 15622, 7812, 3906, 1953, 976, 488,
];

// CORDIC gain scaling factor K = 0.607252935... in Q1.12 format:
// 0.607252935 * 4096 = 2487.27 -> 2487
const CORDIC_GAIN_Q12: i32 = 2487;

/// Computes sine and cosine of an angle (in microradians) using the CORDIC shift-and-add algorithm
fn cordic_cos_sin(mut target_angle: u32) -> (i16, i16) {
    let mut swap_quadrant = false;

    // Normalize second quadrant (>= pi/2) to first quadrant
    // pi/2 in microradians = 1,570,796
    if target_angle >= 1_570_796 {
        target_angle -= 1_570_796;
        swap_quadrant = true;
    }

    // Initial CORDIC vector [X, Y] = [K, 0]
    let mut x = CORDIC_GAIN_Q12;
    let mut y = 0i32;
    let mut current_angle = 0i32;

    for (i, &angle_step) in CORDIC_ANGLES.iter().enumerate() {
        let angle_step = angle_step as i32;
        let x_shift = x >> i;
        let y_shift = y >> i;
        if (target_angle as i32) >= current_angle {
            x -= y_shift;
            y += x_shift;
            current_angle += angle_step;
        } else {
            x += y_shift;
            y -= x_shift;
            current_angle -= angle_step;
        }
    }

    let mut cos_val = x as i16;
    let mut sin_val = y as i16;

    // Apply quadrant symmetry
    if swap_quadrant {
        // cos(theta) = -sin(theta - pi/2)
        // sin(theta) = cos(theta - pi/2)
        let temp = cos_val;
        cos_val = -sin_val;
        sin_val = temp;
    }

    (cos_val, sin_val)
}

/// Generates the fixed-point FFT twiddle factor tables at startup using CORDIC
pub fn build_twiddle_tables() {
    let re_ptr = addr_of_mut!(TWIDDLE_RE) as *mut i16;
    let im_ptr = addr_of_mut!(TWIDDLE_IM) as *mut i16;

    for k in 0..FFT_SIZE / 2 {
        // angle theta = 2 * pi * k / N
        // 2 * pi in microradians = 6,283,185
        let angle = (6_283_185 * k as u64 / FFT_SIZE as u64) as u32;
        let (cos_val, sin_val) = cordic_cos_sin(angle);

        unsafe {
            re_ptr.add(k).write(cos_val);
            im_ptr.add(k).write(-sin_val); // -sin(theta) for e^(-i*theta)
        }
    }
}

impl FixedComplex {
    /// In-place butterfly operation: computes (self, other) = (self + W*other, self - W*other)
    /// Performs Q1.12 scaling and saturates overflows to avoid signal clipping corruption.
    #[inline(always)]
    pub fn butterfly(&mut self, other: &mut FixedComplex, twiddle: FixedComplex) {
        // Complex multiplication in Q1.12:
        // (Tr + i*Ti) * (Or + i*Oi) = (Tr*Or - Ti*Oi) + i * (Tr*Oi + Ti*Or)
        // Shift right by 12 divides out the double scaling factor.
        let t_re =
            ((twiddle.re as i32 * other.re as i32) - (twiddle.im as i32 * other.im as i32)) >> 12;
        let t_im =
            ((twiddle.re as i32 * other.im as i32) + (twiddle.im as i32 * other.re as i32)) >> 12;

        // Additions and Subtractions
        let out_a_re = self.re as i32 + t_re;
        let out_a_im = self.im as i32 + t_im;
        let out_b_re = self.re as i32 - t_re;
        let out_b_im = self.im as i32 - t_im;

        // Saturate overflows to prevent integer wrap corruption
        self.re = out_a_re.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        self.im = out_a_im.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        other.re = out_b_re.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
        other.im = out_b_im.clamp(i16::MIN as i32, i16::MAX as i32) as i16;
    }
}

/// In-place Cooley-Tukey Radix-2 DIT FFT
pub fn fft_inplace(buf: &mut [FixedComplex; FFT_SIZE]) {
    // 1. Bit-reversal permutation (in-place)
    let log2_n = FFT_SIZE.trailing_zeros() as usize; // = 9 for N=512
    for i in 0..FFT_SIZE {
        let j = bit_reverse(i, log2_n);
        if j > i {
            buf.swap(i, j);
        }
    }

    // 2. FFT butterfly iterations
    let mut stage_size = 2;
    let mut twiddle_stride = FFT_SIZE / 2;

    let twiddle_re_ptr = addr_of!(TWIDDLE_RE) as *const i16;
    let twiddle_im_ptr = addr_of!(TWIDDLE_IM) as *const i16;

    while stage_size <= FFT_SIZE {
        let half = stage_size / 2;

        for group_start in (0..FFT_SIZE).step_by(stage_size) {
            for k in 0..half {
                // Read twiddle factors via safe offset reads to avoid borrowing static muts
                let twiddle_idx = k * twiddle_stride;
                let twiddle = unsafe {
                    FixedComplex {
                        re: twiddle_re_ptr.add(twiddle_idx).read(),
                        im: twiddle_im_ptr.add(twiddle_idx).read(),
                    }
                };

                let a_idx = group_start + k;
                let b_idx = group_start + k + half;

                // Split borrow technique: mutably borrow two disjoint indices
                let (left, right) = buf.split_at_mut(b_idx);
                left[a_idx].butterfly(&mut right[0], twiddle);
            }
        }

        stage_size *= 2;
        twiddle_stride /= 2;
    }
}

/// Bit-reversal helper function
fn bit_reverse(mut x: usize, bits: usize) -> usize {
    let mut result = 0;
    for _ in 0..bits {
        result = (result << 1) | (x & 1);
        x >>= 1;
    }
    result
}
