#![cfg_attr(
    any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    ),
    allow(unused_imports)
)]

use core::hint::black_box;

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, pubkey::Pubkey,
};
use svm_math::{
    compound_bounds, compound_lower, compound_upper, exp2_bounds, log2_bounds, pow_bounds,
    defi::{amm, fee, lending, oracle, schedule, staking},
    exp2_lower, exp2_upper, isqrt, log2_lower, log2_upper, mul_div_ceil, mul_div_floor, pow_lower,
    pow_upper, powi_lower, powi_upper, sqrt_ceil, sqrt_floor,
};

entrypoint!(process);

const ITERATIONS: u64 = 100;
#[allow(dead_code, reason = "unused by some single-family size builds")]
const SCALE: u64 = 1_000_000_000;
#[allow(dead_code, reason = "unused by some single-family size builds")]
const WIDE_MUL_B: u64 = 0x8000_0000_0000_3039;
#[allow(dead_code, reason = "unused by some single-family size builds")]
const WIDE_MUL_DENOMINATOR: u64 = u64::MAX;

// A size build is a deliberately separate binary.  Do not allow Cargo feature
// unification to turn one requested family into an all-family measurement.
#[cfg(all(
    feature = "size-wide",
    any(
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-sqrt",
    any(
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-exp2",
    any(
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-log2",
    any(
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-pow",
    any(
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-powi",
    any(
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-compound",
    any(
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-defi_fee",
    any(
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-defi_amm",
    any(
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-defi_lending",
    any(
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )
))]
compile_error!("select at most one size-* feature");
#[cfg(all(
    feature = "size-defi_staking",
    any(feature = "size-defi_oracle", feature = "size-defi_schedule")
))]
compile_error!("select at most one size-* feature");
#[cfg(all(feature = "size-defi_oracle", feature = "size-defi_schedule"))]
compile_error!("select at most one size-* feature");

#[cfg(feature = "size-wide")]
const SIZE_OPERATION: Option<u8> = Some(100);
#[cfg(feature = "size-sqrt")]
const SIZE_OPERATION: Option<u8> = Some(103);
#[cfg(feature = "size-exp2")]
const SIZE_OPERATION: Option<u8> = Some(105);
#[cfg(feature = "size-log2")]
const SIZE_OPERATION: Option<u8> = Some(107);
#[cfg(feature = "size-pow")]
const SIZE_OPERATION: Option<u8> = Some(109);
#[cfg(feature = "size-powi")]
const SIZE_OPERATION: Option<u8> = Some(111);
#[cfg(feature = "size-compound")]
const SIZE_OPERATION: Option<u8> = Some(113);
#[cfg(feature = "size-defi_fee")]
const SIZE_OPERATION: Option<u8> = Some(115);
#[cfg(feature = "size-defi_amm")]
const SIZE_OPERATION: Option<u8> = Some(116);
#[cfg(feature = "size-defi_lending")]
const SIZE_OPERATION: Option<u8> = Some(120);
#[cfg(feature = "size-defi_staking")]
const SIZE_OPERATION: Option<u8> = Some(121);
#[cfg(feature = "size-defi_oracle")]
const SIZE_OPERATION: Option<u8> = Some(124);
#[cfg(feature = "size-defi_schedule")]
const SIZE_OPERATION: Option<u8> = Some(126);
#[cfg(not(any(
    feature = "size-wide",
    feature = "size-sqrt",
    feature = "size-exp2",
    feature = "size-log2",
    feature = "size-pow",
    feature = "size-powi",
    feature = "size-compound",
    feature = "size-defi_fee",
    feature = "size-defi_amm",
    feature = "size-defi_lending",
    feature = "size-defi_staking",
    feature = "size-defi_oracle",
    feature = "size-defi_schedule"
)))]
const SIZE_OPERATION: Option<u8> = None;

pub fn iters(_operation: u8) -> u64 {
    ITERATIONS
}

// Dispatch exactly once per instruction. Each arm expands its loop locally so
// a benchmarked iteration never pays for target/control selection or a helper
// call that could perturb its legacy SBF baseline.
macro_rules! dispatch_fold {
    ($operation:expr, $iterations:expr, { $($pattern:pat => |$index:ident| $body:expr),+ $(,)? }) => {
        match $operation {
            $($pattern => {
                let mut accumulator = 0u64;
                for raw_index in 0..$iterations {
                    let $index = black_box(raw_index);
                    accumulator ^= $body;
                }
                let _ = black_box(accumulator);
            },)+
            _ => unreachable!(),
        }
    };
}

#[inline(always)]
fn empty(iterations: u64, seed: u64) {
    let mut value = seed;
    for _ in 0..iterations {
        value = black_box(value);
    }
    let _ = black_box(value);
}

fn seed64(data: &[u8]) -> u64 {
    let mut seed = 0x9E37_79B9_7F4A_7C15u64;
    for (index, byte) in data.iter().enumerate().take(8) {
        seed ^= (*byte as u64) << (8 * index);
    }
    seed | 1
}

pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let operation = SIZE_OPERATION.unwrap_or_else(|| data.first().copied().unwrap_or(0));
    let seed = black_box(seed64(data.get(1..).unwrap_or_default()));
    let iterations = iters(operation);

    match operation {
        0 => empty(iterations, seed),
        30 => libcall_self_check(seed),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-wide"
        ))]
        100 | 101 | 128 | 130 | 131 | 158 => wide(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-sqrt"
        ))]
        102..=104 | 132..=134 => sqrt(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-exp2"
        ))]
        105 | 106 | 129 | 135 | 136 | 159 => exp2(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-log2"
        ))]
        107 | 108 | 137 | 138 | 160 | 190 => log2(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-pow"
        ))]
        109 | 110 | 139 | 140 | 161 | 191 => pow(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-powi"
        ))]
        111 | 112 | 141 | 142 => powi(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-compound"
        ))]
        113 | 114 | 143 | 144 | 162 | 192 => compound(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-defi_fee"
        ))]
        115 | 145 => defi_fee(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-defi_amm"
        ))]
        116..=118 | 146..=148 => defi_amm(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-defi_lending"
        ))]
        119 | 120 | 149 | 150 => defi_lending(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-defi_staking"
        ))]
        121..=123 | 151..=153 | 163 | 193 => defi_staking(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-defi_oracle"
        ))]
        124 | 154 => defi_oracle(operation, iterations),
        #[cfg(any(
            not(any(
                feature = "size-wide",
                feature = "size-sqrt",
                feature = "size-exp2",
                feature = "size-log2",
                feature = "size-pow",
                feature = "size-powi",
                feature = "size-compound",
                feature = "size-defi_fee",
                feature = "size-defi_amm",
                feature = "size-defi_lending",
                feature = "size-defi_staking",
                feature = "size-defi_oracle",
                feature = "size-defi_schedule"
            )),
            feature = "size-defi_schedule"
        ))]
        125..=127 | 155..=157 => defi_schedule(operation, iterations),
        _ => {}
    }
    Ok(())
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-wide"
))]
fn wide(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        100 => |index| {
            let a = black_box(1_000_000 + index);
            let b = black_box(997);
            let denominator = black_box(1_003);
            mul_div_floor(a, b, denominator).unwrap()
        },
        101 => |index| {
            let a = black_box(1_000_000 + index);
            let b = black_box(997);
            let denominator = black_box(1_003);
            mul_div_ceil(a, b, denominator).unwrap()
        },
        128 => |index| {
            let a = black_box(u64::MAX - index);
            let b = black_box(WIDE_MUL_B);
            let denominator = black_box(WIDE_MUL_DENOMINATOR);
            mul_div_floor(a, b, denominator).unwrap()
        },
        130 | 131 => |index| {
            let a = black_box(1_000_000 + index);
            let b = black_box(997);
            let denominator = black_box(1_003);
            Ok::<_, svm_math::MathError>(a ^ b ^ denominator).unwrap()
        },
        158 => |index| {
            let a = black_box(u64::MAX - index);
            let b = black_box(WIDE_MUL_B);
            let denominator = black_box(WIDE_MUL_DENOMINATOR);
            Ok::<_, svm_math::MathError>(a ^ b ^ denominator).unwrap()
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-sqrt"
))]
fn sqrt(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        102 => |index| {
            let value = black_box(u128::from(1_000_000 + index) * 1_000_003);
            isqrt(value) as u64
        },
        132 => |index| {
            let value = black_box(u128::from(1_000_000 + index) * 1_000_003);
            value as u64
        },
        103 => |index| {
            let value = black_box(2_000_000_000 + index);
            sqrt_floor(value, SCALE).unwrap()
        },
        133 => |index| {
            let value = black_box(2_000_000_000 + index);
            Ok::<_, svm_math::MathError>(value).unwrap()
        },
        104 => |index| {
            let value = black_box(2_000_000_000 + index);
            sqrt_ceil(value, SCALE).unwrap()
        },
        134 => |index| {
            let value = black_box(2_000_000_000 + index);
            Ok::<_, svm_math::MathError>(value).unwrap()
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-exp2"
))]
fn exp2(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        105 => |index| {
            let value = black_box(500_000_000 + index) as i64;
            exp2_lower(value, SCALE).unwrap()
        },
        135 => |index| {
            let value = black_box(500_000_000 + index) as i64;
            Ok::<_, svm_math::MathError>(value).unwrap() as u64
        },
        106 => |index| {
            let value = black_box(500_000_000 + index) as i64;
            exp2_upper(value, SCALE).unwrap()
        },
        136 => |index| {
            let value = black_box(500_000_000 + index) as i64;
            Ok::<_, svm_math::MathError>(value).unwrap() as u64
        },
        129 => |index| {
            let value = black_box(500_000_000 + index) as i64;
            let (lower, upper) = exp2_bounds(value, SCALE).unwrap();
            lower ^ upper
        },
        159 => |index| {
            let value = black_box(500_000_000 + index) as i64;
            Ok::<_, svm_math::MathError>(value).unwrap() as u64
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-log2"
))]
fn log2(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        107 => |index| {
            let value = black_box(2 + index % 2);
            log2_lower(value, 1).unwrap() as u64
        },
        160 => |index| {
            let value = black_box(977_000_000_000 + index);
            let (lower, upper) = log2_bounds(value, SCALE).unwrap();
            (lower as u64) ^ (upper as u64)
        },
        190 => |index| {
            let value = black_box(977_000_000_000 + index);
            Ok::<_, svm_math::MathError>(value).unwrap()
        },
        137 => |index| {
            let value = black_box(2 + index % 2);
            let _ = black_box(value);
            Ok::<_, svm_math::MathError>(1_i64).unwrap() as u64
        },
        108 => |index| {
            let value = black_box(2 + index % 2);
            log2_upper(value, 1).unwrap() as u64
        },
        138 => |index| {
            let value = black_box(2 + index % 2);
            let _ = black_box(value);
            Ok::<_, svm_math::MathError>(1_i64).unwrap() as u64
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-pow"
))]
fn pow(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        109 => |index| {
            let value = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            pow_lower(value, exponent, SCALE).unwrap()
        },
        161 => |index| {
            let value = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            let (lower, upper) = pow_bounds(value, exponent, SCALE).unwrap();
            lower ^ upper
        },
        191 => |index| {
            let value = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            let _ = black_box(exponent);
            Ok::<_, svm_math::MathError>(value).unwrap()
        },
        139 => |index| {
            let value = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            let _ = black_box(exponent);
            Ok::<_, svm_math::MathError>(value).unwrap()
        },
        110 => |index| {
            let value = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            pow_upper(value, exponent, SCALE).unwrap()
        },
        140 => |index| {
            let value = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            let _ = black_box(exponent);
            Ok::<_, svm_math::MathError>(value).unwrap()
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-powi"
))]
fn powi(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        111 => |index| {
            let value = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            powi_lower(value, exponent, SCALE).unwrap()
        },
        141 => |index| {
            let value = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            let _ = black_box(exponent);
            Ok::<_, svm_math::MathError>(value).unwrap()
        },
        112 => |index| {
            let value = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            powi_upper(value, exponent, SCALE).unwrap()
        },
        142 => |index| {
            let value = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            let _ = black_box(exponent);
            Ok::<_, svm_math::MathError>(value).unwrap()
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-compound"
))]
fn compound(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        113 => |index| {
            let rate = 70_000_000;
            let periods_per_year = 63_072_000;
            let elapsed_periods = 63_072_000 + index;
            compound_lower(rate, periods_per_year, elapsed_periods, SCALE).unwrap()
        },
        162 => |index| {
            let rate = 70_000_000;
            let periods_per_year = 63_072_000;
            let elapsed_periods = 63_072_000 + index;
            let (lower, upper) =
                compound_bounds(rate, periods_per_year, elapsed_periods, SCALE).unwrap();
            lower ^ upper
        },
        192 => |index| {
            let elapsed_periods = black_box(63_072_000 + index);
            Ok::<_, svm_math::MathError>(elapsed_periods).unwrap()
        },
        114 => |index| {
            let rate = 70_000_000;
            let periods_per_year = 63_072_000;
            let elapsed_periods = 63_072_000 + index;
            compound_upper(rate, periods_per_year, elapsed_periods, SCALE).unwrap()
        },
        143 => |index| {
            let rate = 70_000_000;
            let _periods_per_year = 63_072_000;
            let elapsed_periods = 63_072_000 + index;
            Ok::<_, svm_math::MathError>(rate).unwrap() ^ elapsed_periods
        },
        144 => |index| {
            let rate = 70_000_000;
            let _periods_per_year = 63_072_000;
            let elapsed_periods = 63_072_000 + index;
            Ok::<_, svm_math::MathError>(rate).unwrap() ^ elapsed_periods
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-defi_fee"
))]
fn defi_fee(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        115 => |index| {
            let amount = black_box(1_000_000 + index);
            let rate = black_box(30_u16);
            let (net, fee) = fee::net_of_fee(amount, rate).unwrap();
            net ^ fee
        },
        145 => |index| {
            let amount = black_box(1_000_000 + index);
            let rate = black_box(30_u16);
            Ok::<_, svm_math::MathError>((amount, rate))
                .unwrap()
                .0
                ^ u64::from(rate)
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-defi_amm"
))]
fn defi_amm(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        116 => |index| {
            let reserve_in = 50_000_000 + index;
            let reserve_out = 80_000_000_000;
            let amount = 1_000_000 + index;
            let rate = 30_u16;
            amm::quote_exact_in(reserve_in, reserve_out, amount, rate).unwrap()
        },
        146 => |index| {
            let reserve_in = 50_000_000 + index;
            let reserve_out = 80_000_000_000;
            let amount = 1_000_000 + index;
            let rate = 30_u16;
            Ok::<_, svm_math::MathError>(
                reserve_in ^ reserve_out ^ amount ^ u64::from(rate),
            )
            .unwrap()
        },
        117 => |index| {
            let reserve_in = 50_000_000 + index;
            let reserve_out = 80_000_000_000;
            let amount = 1_000_000_000 + index;
            let rate = 30_u16;
            amm::quote_exact_out(reserve_in, reserve_out, amount, rate).unwrap()
        },
        147 => |index| {
            let reserve_in = 50_000_000 + index;
            let reserve_out = 80_000_000_000;
            let amount = 1_000_000_000 + index;
            let rate = 30_u16;
            Ok::<_, svm_math::MathError>(
                reserve_in ^ reserve_out ^ amount ^ u64::from(rate),
            )
            .unwrap()
        },
        118 => |index| {
            let base = 4_000_000 + index;
            let quote = 9_000_000;
            amm::initial_lp_shares_floor(base, quote)
        },
        148 => |index| {
            let base = 4_000_000 + index;
            let quote = 9_000_000;
            base ^ quote
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-defi_lending"
))]
fn defi_lending(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        119 => |index| {
            let borrowed = black_box(70_000_000 + index);
            let supplied = black_box(100_000_000);
            lending::utilization_bps(borrowed, supplied).unwrap()
        },
        149 => |index| {
            let borrowed = black_box(70_000_000 + index);
            let supplied = black_box(100_000_000);
            Ok::<_, svm_math::MathError>(borrowed ^ supplied).unwrap()
        },
        120 => |index| {
            let config = black_box((100, 500, 2_000, 8_000));
            let utilization = black_box(8_001 + index % 100);
            lending::borrow_rate_bps(
                utilization,
                config.0,
                config.1,
                config.2,
                config.3,
            )
            .unwrap()
        },
        150 => |index| {
            let config = black_box((100, 500, 2_000, 8_000));
            let utilization = black_box(8_001 + index % 100);
            (Ok::<_, svm_math::MathError>(config).unwrap() == config) as u64 ^ utilization
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-defi_staking"
))]
fn defi_staking(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        121 => |index| {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            staking::reward_index_accrue_lower(index_value, reward, stake, SCALE).unwrap()
                ^ reward
                ^ stake
        },
        122 => |index| {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            staking::reward_index_accrue_upper(index_value, reward, stake, SCALE).unwrap()
                ^ reward
                ^ stake
        },
        151 => |index| {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            Ok::<_, svm_math::MathError>((index_value, reward, stake))
                .unwrap()
                .0
                ^ reward
                ^ stake
        },
        152 => |index| {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            Ok::<_, svm_math::MathError>((index_value, reward, stake))
                .unwrap()
                .0
                ^ reward
                ^ stake
        },
        163 => |index| {
            let index_lower = black_box(1_250_000_000);
            let index_upper = black_box(1_250_001_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            let (lower, upper) =
                staking::reward_index_accrue(index_lower, index_upper, reward, stake, SCALE)
                    .unwrap();
            lower ^ upper
        },
        193 => |index| {
            let index_lower = black_box(1_250_000_000);
            let index_upper = black_box(1_250_001_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            let _ = black_box((index_lower, index_upper, stake));
            Ok::<_, svm_math::MathError>(reward).unwrap()
        },
        123 => |index| {
            let staked = black_box(100_000_000 + index);
            let now = black_box(1_300_000_000);
            let snapshot = black_box(1_250_000_000);
            staking::rewards_owed_floor(staked, now, snapshot, SCALE).unwrap()
        },
        153 => |index| {
            let staked = black_box(100_000_000 + index);
            let now = black_box(1_300_000_000);
            let snapshot = black_box(1_250_000_000);
            let _ = black_box((now, snapshot));
            Ok::<_, svm_math::MathError>(staked).unwrap()
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-defi_oracle"
))]
fn defi_oracle(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        124 => |index| {
            let price = black_box(20_000_000 + index as i64);
            let confidence = black_box(100_000u64);
            let exponent = black_box(-6i32);
            let (lower, upper) =
                oracle::price_bounds_scaled(price, confidence, exponent, SCALE).unwrap();
            lower ^ upper
        },
        154 => |index| {
            let price = black_box(20_000_000 + index as i64);
            let confidence = black_box(100_000u64);
            let exponent = black_box(-6i32);
            let (price, confidence, exponent) =
                Ok::<_, svm_math::MathError>((price, confidence, exponent)).unwrap();
            price as u64 ^ confidence ^ exponent.unsigned_abs() as u64
        }
    });
}

#[cfg(any(
    not(any(
        feature = "size-wide",
        feature = "size-sqrt",
        feature = "size-exp2",
        feature = "size-log2",
        feature = "size-pow",
        feature = "size-powi",
        feature = "size-compound",
        feature = "size-defi_fee",
        feature = "size-defi_amm",
        feature = "size-defi_lending",
        feature = "size-defi_staking",
        feature = "size-defi_oracle",
        feature = "size-defi_schedule"
    )),
    feature = "size-defi_schedule"
))]
fn defi_schedule(operation: u8, iterations: u64) {
    dispatch_fold!(operation, iterations, {
        125 => |index| {
            let config = black_box((1_000, 1_500, 10_000));
            let total = black_box(1_000_000_000);
            let now = black_box(2_000 + index);
            schedule::vested_floor(total, config.0, config.1, config.2, now).unwrap()
        },
        155 => |index| {
            let config = black_box((1_000, 1_500, 10_000));
            let total = black_box(1_000_000_000);
            let now = black_box(2_000 + index);
            let _ = black_box(config);
            Ok::<_, svm_math::MathError>((total, now)).unwrap().0 ^ now
        },
        126 => |index| {
            let from = black_box(1_000_000);
            let to = black_box(2_000_000);
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            schedule::linear_interp_floor(from, to, elapsed, duration).unwrap()
        },
        156 => |index| {
            let from = black_box(1_000_000);
            let to = black_box(2_000_000);
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            Ok::<_, svm_math::MathError>((from, to, elapsed, duration))
                .unwrap()
                .0
                ^ to
                ^ elapsed
                ^ duration
        },
        127 => |index| {
            let from = black_box(2_000_000);
            let to = black_box(1_000_000);
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            schedule::linear_interp_ceil(from, to, elapsed, duration).unwrap()
        },
        157 => |index| {
            let from = black_box(2_000_000);
            let to = black_box(1_000_000);
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            Ok::<_, svm_math::MathError>((from, to, elapsed, duration))
                .unwrap()
                .0
                ^ to
                ^ elapsed
                ^ duration
        }
    });
}

fn libcall_self_check(seed: u64) {
    let mut pairs = [
        (u128::MAX, 1u128),
        (u128::MAX, u64::MAX as u128),
        (u128::MAX, (1u128 << 64) + 1),
        ((1u128 << 127) | 1, (1u128 << 63) + 1),
        (1u128 << 64, 1u128 << 64),
        (0, 7),
    ];
    pairs[0].0 ^= (seed & 0xFFFF) as u128;
    for (numerator, denominator) in pairs {
        let quotient = black_box(numerator) / black_box(denominator);
        let remainder = numerator % denominator;
        assert!(remainder < denominator);
        assert_eq!(
            quotient
                .checked_mul(denominator)
                .and_then(|value| value.checked_add(remainder)),
            Some(numerator)
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{ITERATIONS, WIDE_MUL_B, WIDE_MUL_DENOMINATOR};

    #[test]
    fn wide_mul_div_workload_forces_the_128_by_64_path() {
        for index in 0..ITERATIONS {
            let a = u64::MAX - index;
            let product = u128::from(a) * u128::from(WIDE_MUL_B);
            let high = (product >> 64) as u64;
            assert_ne!(high, 0);
            assert!(high < WIDE_MUL_DENOMINATOR);
            assert_eq!(
                svm_math::mul_div_floor(a, WIDE_MUL_B, WIDE_MUL_DENOMINATOR),
                Ok((product / u128::from(WIDE_MUL_DENOMINATOR)) as u64)
            );
        }
    }
}
