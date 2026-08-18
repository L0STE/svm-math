use core::hint::black_box;

const Q_BITS: u32 = 61;
const Q: u64 = 1_u64 << Q_BITS;
const LN2_Q64_LOWER: u64 = 0xB172_17F7_D1CF_79AB;
const LN2_Q64_UPPER: u64 = 0xB172_17F7_D1CF_79AC;
const RECIPROCAL_TABLE: [u16; 256] = reciprocal_table();

const fn reciprocal_table() -> [u16; 256] {
    let mut table = [0_u16; 256];
    let mut index = 0;
    while index < table.len() {
        table[index] = (((1_u32 << 19) - 3 * (1 << 8)) / (256 + index as u32)) as u16;
        index += 1;
    }
    table
}

#[inline(always)]
const fn pair_mul(a: u64, b: u64) -> (u64, u64) {
    let (a_high, a_low) = (a >> 32, a & 0xffff_ffff);
    let (b_high, b_low) = (b >> 32, b & 0xffff_ffff);
    let low_low = a_low * b_low;
    let low_high = a_low * b_high;
    let high_low = a_high * b_low;
    let high_high = a_high * b_high;
    let middle = (low_low >> 32) + (low_high & 0xffff_ffff) + (high_low & 0xffff_ffff);
    let low = (middle << 32) | (low_low & 0xffff_ffff);
    let high = high_high + (low_high >> 32) + (high_low >> 32) + (middle >> 32);
    (high, low)
}

#[inline(always)]
fn multiply_high(a: u64, b: u64) -> u64 {
    pair_mul(a, b).0
}

const fn reciprocal(normalized_denominator: u64) -> u64 {
    let low_bit = normalized_denominator & 1;
    let top_nine = normalized_denominator >> 55;
    let top_forty = (normalized_denominator >> 24) + 1;
    let rounded_half = (normalized_denominator >> 1) + low_bit;
    let seed = RECIPROCAL_TABLE[(top_nine - 256) as usize] as u64;
    let first = (seed << 11) - (seed * seed * top_forty >> 40) - 1;
    let second = (first << 13) + (first * ((1_u64 << 60) - first * top_forty) >> 47);
    let product_low = second.wrapping_mul(rounded_half);
    let error = ((second >> 1) & low_bit.wrapping_neg()).wrapping_sub(product_low);
    let third = (second << 31).wrapping_add(pair_mul(second, error).0 >> 1);
    let (product_high, product_low) = pair_mul(third, normalized_denominator);
    let carry = product_low.overflowing_add(normalized_denominator).1;
    third
        .wrapping_sub(product_high.wrapping_add(carry as u64))
        .wrapping_sub(normalized_denominator)
}

const fn prepared(scale: u64) -> (u64, u64, u32) {
    let shift = scale.leading_zeros();
    let normalized = scale << shift;
    (normalized, reciprocal(normalized), shift)
}

const PREPARED_1E18: (u64, u64, u32) = prepared(1_000_000_000_000_000_000);

#[inline(always)]
fn divide_with(high: u64, low: u64, denominator: u64, reciprocal: u64) -> (u64, u64) {
    let (product_high, product_low) = pair_mul(reciprocal, high);
    let (quotient_low, carry) = product_low.overflowing_add(low);
    let mut quotient = product_high
        .wrapping_add(high)
        .wrapping_add(u64::from(carry))
        .wrapping_add(1);
    let mut remainder = low.wrapping_sub(quotient.wrapping_mul(denominator));
    if remainder > quotient_low {
        quotient = quotient.wrapping_sub(1);
        remainder = remainder.wrapping_add(denominator);
    }
    if remainder >= denominator {
        quotient = quotient.wrapping_add(1);
        remainder -= denominator;
    }
    (quotient, remainder)
}

#[inline(always)]
fn divide_runtime(high: u64, low: u64, denominator: u64) -> (u64, u64) {
    let shift = denominator.leading_zeros();
    let normalized = denominator << shift;
    let normalized_high = (high << shift) | (low >> (64 - shift));
    let normalized_low = low << shift;
    let (quotient, remainder) = divide_with(
        normalized_high,
        normalized_low,
        normalized,
        reciprocal(normalized),
    );
    (quotient, remainder >> shift)
}

#[inline(always)]
fn divide_prepared(high: u64, low: u64) -> (u64, u64) {
    let (normalized, reciprocal, shift) = PREPARED_1E18;
    let normalized_high = (high << shift) | (low >> (64 - shift));
    let normalized_low = low << shift;
    let (quotient, remainder) =
        divide_with(normalized_high, normalized_low, normalized, reciprocal);
    (quotient, remainder >> shift)
}

pub fn decimal_whole_remainder(value: u64, scale: u64) -> u64 {
    value / scale ^ value % scale
}

pub fn decimal_wide_runtime(value: u64, scale: u64) -> u64 {
    let remainder = value % scale;
    let (high, low) = pair_mul(remainder, Q);
    let (quotient, remainder) = divide_runtime(high, low, scale);
    quotient ^ remainder
}

pub fn decimal_wide_prepared(value: u64, scale: u64) -> u64 {
    let remainder = value % scale;
    let (high, low) = pair_mul(remainder, Q);
    let (quotient, remainder) = divide_prepared(high, low);
    debug_assert_eq!(scale, 1_000_000_000_000_000_000);
    quotient ^ remainder
}

pub fn normalize_runtime(value: u64, scale: u64) -> u64 {
    let high = value >> 1;
    let low = value << 63;
    let (quotient, remainder) = divide_runtime(high, low, scale);
    quotient ^ remainder
}

pub fn normalize_prepared(value: u64, scale: u64) -> u64 {
    let high = value >> 1;
    let low = value << 63;
    let (quotient, remainder) = divide_prepared(high, low);
    debug_assert_eq!(scale, 1_000_000_000_000_000_000);
    quotient ^ remainder
}

pub fn widening_pair(a: u64, b: u64) -> u64 {
    let (high, low) = pair_mul(a, b);
    high ^ low
}

pub fn widening_native(a: u64, b: u64) -> u64 {
    let product = u128::from(a) * u128::from(b);
    (product >> 64) as u64 ^ product as u64
}

pub fn entry_old<const UPPER: bool>(value: u64, scale: u64) -> u64 {
    let product = u128::from(value) * u128::from(Q);
    let quotient = product / u128::from(scale);
    let remainder = product % u128::from(scale);
    (quotient + u128::from(UPPER && remainder != 0)) as u64
}

pub fn entry_decomposed<const UPPER: bool>(value: u64, scale: u64) -> u64 {
    let whole = value / scale;
    let remainder = value % scale;
    let quotient = (Q / scale) * remainder;
    let tail = (Q % scale) * remainder;
    whole * Q + quotient + tail / scale + u64::from(UPPER && tail % scale != 0)
}

const fn isqrt_u128(value: u128) -> u128 {
    if value < 2 {
        return value;
    }
    let mut estimate = 1_u128 << (128 - value.leading_zeros()).div_ceil(2);
    loop {
        let next = (estimate + value / estimate) / 2;
        if next >= estimate {
            return estimate;
        }
        estimate = next;
    }
}

const fn exp2_fraction_bits(upper: bool) -> [u64; 64] {
    let mut table = [0_u64; 64];
    let mut previous = 0_u64;
    let mut index = 0;
    while index < 64 {
        let square = if index == 0 {
            1_u128 << 127
        } else {
            (previous as u128) << 63
        };
        let root = isqrt_u128(square);
        table[index] = if upper && root * root != square {
            (root + 1) as u64
        } else {
            root as u64
        };
        previous = table[index];
        index += 1;
    }
    table
}

const fn const_mul_q63(a: u64, b: u64, upper: bool) -> u64 {
    let product = a as u128 * b as u128;
    let mut result = (product >> 63) as u64;
    if upper && product & ((1_u128 << 63) - 1) != 0 {
        result += 1;
    }
    result
}

const fn exp2_table(upper: bool) -> [u64; 2048] {
    let factors = exp2_fraction_bits(upper);
    let mut table = [0_u64; 2048];
    let mut index = 0;
    while index < 2048 {
        let mut q63 = 1_u64 << 63;
        let mut bit = 0;
        while bit < 11 {
            if index & (1 << bit) != 0 {
                q63 = const_mul_q63(q63, factors[10 - bit], upper);
            }
            bit += 1;
        }
        table[index] = q63;
        index += 1;
    }
    table
}

static EXP2_LOWER: [u64; 2048] = exp2_table(false);
static EXP2_UPPER: [u64; 2048] = exp2_table(true);

pub fn exp2_core<const UPPER: bool>(exponent_q: i128) -> (u64, i128) {
    let integer = exponent_q.div_euclid(i128::from(Q));
    let fraction = exponent_q.rem_euclid(i128::from(Q)) as u64;
    if fraction == 0 {
        return (1_u64 << 63, integer);
    }
    let fraction_q64 = fraction << 3;
    let index = (fraction_q64 >> 53) as usize;
    let residual = fraction_q64 << 11;
    let ln2 = if UPPER { LN2_Q64_UPPER } else { LN2_Q64_LOWER };
    let y = multiply_high(residual, ln2);
    let y2 = multiply_high(y, y) >> 11;
    let y3 = multiply_high(y2, y) >> 11;
    let y4 = multiply_high(y2, y2) >> 11;
    let series = if UPPER {
        y + (y2 / 2 + 1) + (y3 / 6 + 1) + (y4 / 24 + 1) + 1_408
    } else {
        y + y2 / 2 + y3 / 6 + y4 / 24
    };
    let base = if UPPER { EXP2_UPPER[index & 2047] } else { EXP2_LOWER[index & 2047] };
    let correction = multiply_high(base, series) >> 11;
    let (mantissa, carry0) = base.overflowing_add(correction);
    let (mantissa, carry1) = mantissa.overflowing_add(if UPPER { 2 } else { 0 });
    if carry0 || carry1 {
        (
            (1_u64 << 63) + (mantissa >> 1) + u64::from(UPPER && mantissa & 1 != 0),
            integer + 1,
        )
    } else {
        (mantissa, integer)
    }
}

#[inline(always)]
fn project_q63<const UPPER: bool>(mantissa: u64, integer: i128, scale: u64) -> u64 {
    let shift = 63_i128 - integer;
    let product = u128::from(mantissa) * u128::from(scale);
    let quotient = product >> shift;
    let discarded = product & ((1_u128 << shift) - 1) != 0;
    quotient as u64 + u64::from(UPPER && discarded)
}

#[inline(always)]
fn project_old_q61<const UPPER: bool>(mantissa: u64, integer: i128, scale: u64) -> u64 {
    let mantissa = (mantissa >> 2) + u64::from(UPPER && mantissa & 3 != 0);
    let shift = 61_i128 - integer;
    let product = u128::from(mantissa) * u128::from(scale);
    let quotient = product >> shift;
    let discarded = product & ((1_u128 << shift) - 1) != 0;
    quotient as u64 + u64::from(UPPER && discarded)
}

pub fn exit_old<const UPPER: bool>(mantissa: u64, integer: i128, scale: u64) -> u64 {
    project_old_q61::<UPPER>(mantissa, integer, scale)
}

pub fn exit_direct<const UPPER: bool>(mantissa: u64, integer: i128, scale: u64) -> u64 {
    project_q63::<UPPER>(mantissa, integer, scale)
}

pub fn exp2_old_projection<const UPPER: bool>(value: u64, scale: u64) -> u64 {
    let exponent_q = entry_decomposed::<UPPER>(value, scale);
    let (mantissa, integer) = exp2_core::<UPPER>(i128::from(exponent_q));
    exit_old::<UPPER>(mantissa, integer, scale)
}

pub fn exp2_direct_projection<const UPPER: bool>(value: u64, scale: u64) -> u64 {
    let exponent_q = entry_decomposed::<UPPER>(value, scale);
    let (mantissa, integer) = exp2_core::<UPPER>(i128::from(exponent_q));
    exit_direct::<UPPER>(mantissa, integer, scale)
}

pub fn powi_repeated<const UPPER: bool>(mut base: u64, mut exponent: u64, scale: u64) -> u64 {
    let mut result = scale;
    while exponent != 0 {
        if exponent & 1 != 0 {
            result = if UPPER {
                svm_math::mul_div_ceil(result, base, scale).unwrap()
            } else {
                svm_math::mul_div_floor(result, base, scale).unwrap()
            };
        }
        exponent >>= 1;
        if exponent != 0 {
            base = if UPPER {
                svm_math::mul_div_ceil(base, base, scale).unwrap()
            } else {
                svm_math::mul_div_floor(base, base, scale).unwrap()
            };
        }
    }
    result
}

pub fn workload(operation: u8, index: u64) -> u64 {
    match operation {
        200 => widening_pair(black_box(u64::MAX - index), black_box(0xfedc_ba98_7654_3211)),
        201 => widening_native(black_box(u64::MAX - index), black_box(0xfedc_ba98_7654_3211)),
        230 => black_box(u64::MAX - index) ^ black_box(0xfedc_ba98_7654_3211),
        202 => entry_old::<false>(black_box(500_000_000 + index), black_box(1_000_000_000)),
        203 => entry_decomposed::<false>(black_box(500_000_000 + index), black_box(1_000_000_000)),
        232 => black_box(500_000_000 + index) ^ black_box(1_000_000_000),
        204 => {
            let (mantissa, integer) = exp2_core::<false>(black_box(i128::from(Q / 2 + index)));
            mantissa ^ integer as u64
        }
        234 => black_box(Q / 2 + index),
        205 | 206 | 213 | 214 => {
            let mantissa = black_box(0xb504_f333_f9de_6484_u64.wrapping_add(index));
            let integer = black_box(0_i128);
            match operation {
                205 => exit_old::<false>(black_box(mantissa), black_box(integer), black_box(1_000_000_000)),
                206 => exit_direct::<false>(black_box(mantissa), black_box(integer), black_box(1_000_000_000)),
                213 => exit_old::<true>(black_box(mantissa), black_box(integer), black_box(1_000_000_000)),
                _ => exit_direct::<true>(black_box(mantissa), black_box(integer), black_box(1_000_000_000)),
            }
        }
        235 | 243 => black_box(Q / 2 + index) ^ black_box(1_000_000_000),
        207 => exp2_old_projection::<false>(black_box(500_000_000 + index), black_box(1_000_000_000)),
        208 => exp2_direct_projection::<false>(black_box(500_000_000 + index), black_box(1_000_000_000)),
        215 => exp2_old_projection::<true>(black_box(500_000_000 + index), black_box(1_000_000_000)),
        216 => exp2_direct_projection::<true>(black_box(500_000_000 + index), black_box(1_000_000_000)),
        237 | 245 => black_box(500_000_000 + index) ^ black_box(1_000_000_000),
        209 => powi_repeated::<false>(black_box(9_000_100_000_000_000_000 + index), black_box(31), black_box(9_000_000_000_000_000_000)),
        210 => svm_math::powi_lower(black_box(9_000_100_000_000_000_000 + index), black_box(31), black_box(9_000_000_000_000_000_000)).unwrap(),
        211 => powi_repeated::<true>(black_box(9_000_100_000_000_000_000 + index), black_box(31), black_box(9_000_000_000_000_000_000)),
        212 => svm_math::powi_upper(black_box(9_000_100_000_000_000_000 + index), black_box(31), black_box(9_000_000_000_000_000_000)).unwrap(),
        239 | 241 => black_box(9_000_100_000_000_000_000 + index) ^ black_box(31) ^ black_box(9_000_000_000_000_000_000),
        217 => svm_math::exp2_lower(black_box(500_000_000_000_000_000_i64), black_box(1_000_000_000_000_000_000)).unwrap(),
        218 => svm_math::exp2_upper(black_box(500_000_000_000_000_000_i64), black_box(1_000_000_000_000_000_000)).unwrap(),
        246 => black_box(500_000_000_000_000_000_u64) ^ black_box(1_000_000_000_000_000_000),
        219 => svm_math::log2_lower(black_box(2_000_000_000_000_000_000), black_box(1_000_000_000_000_000_000)).unwrap() as u64,
        220 => svm_math::log2_upper(black_box(2_000_000_000_000_000_000), black_box(1_000_000_000_000_000_000)).unwrap() as u64,
        247 => black_box(2_000_000_000_000_000_000) ^ black_box(1_000_000_000_000_000_000),
        221 => svm_math::pow_lower(black_box(2_000_000_000_000_000_000), black_box(500_000_000_000_000_000), black_box(1_000_000_000_000_000_000)).unwrap(),
        222 => svm_math::pow_upper(black_box(2_000_000_000_000_000_000), black_box(500_000_000_000_000_000), black_box(1_000_000_000_000_000_000)).unwrap(),
        248 => black_box(2_000_000_000_000_000_000) ^ black_box(500_000_000_000_000_000) ^ black_box(1_000_000_000_000_000_000),
        223 => svm_math::powi_lower(black_box(1_000_100_000_000_000_000 + index), black_box(10), black_box(1_000_000_000_000_000_000)).unwrap(),
        224 => svm_math::powi_upper(black_box(1_000_100_000_000_000_000 + index), black_box(10), black_box(1_000_000_000_000_000_000)).unwrap(),
        249 => black_box(1_000_100_000_000_000_000 + index) ^ black_box(10) ^ black_box(1_000_000_000_000_000_000),
        225 => svm_math::compound_lower(black_box(70_000_000_000_000_000), black_box(63_072_000), black_box(63_072_000 + index), black_box(1_000_000_000_000_000_000)).unwrap(),
        226 => svm_math::compound_upper(black_box(70_000_000_000_000_000), black_box(63_072_000), black_box(63_072_000 + index), black_box(1_000_000_000_000_000_000)).unwrap(),
        250 => black_box(70_000_000_000_000_000) ^ black_box(63_072_000) ^ black_box(63_072_000 + index) ^ black_box(1_000_000_000_000_000_000),
        227 => decimal_whole_remainder(black_box(1_500_000_000_000_000_000 + index), black_box(1_000_000_000_000_000_000)),
        228 => decimal_wide_runtime(black_box(1_500_000_000_000_000_000 + index), black_box(1_000_000_000_000_000_000)),
        229 => decimal_wide_prepared(black_box(1_500_000_000_000_000_000 + index), black_box(1_000_000_000_000_000_000)),
        251 | 252 => black_box(1_500_000_000_000_000_000 + index) ^ black_box(1_000_000_000_000_000_000),
        231 => normalize_runtime(black_box(750_000_000_000_000_000 + index), black_box(1_000_000_000_000_000_000)),
        233 => normalize_prepared(black_box(750_000_000_000_000_000 + index), black_box(1_000_000_000_000_000_000)),
        253 => black_box(750_000_000_000_000_000 + index) ^ black_box(1_000_000_000_000_000_000),
        _ => 0,
    }
}
