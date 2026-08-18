#[cfg(all(feature = "current", feature = "old"))]
compile_error!("select exactly one of `current` or `old`");
#[cfg(not(any(feature = "current", feature = "old")))]
compile_error!("select exactly one of `current` or `old`");

use core::hint::black_box;

use solana_program::{
    account_info::AccountInfo, entrypoint, entrypoint::ProgramResult, program::set_return_data,
    pubkey::Pubkey,
};

#[cfg(feature = "current")]
#[path = "adapter_current.rs"]
mod adapter;
#[cfg(feature = "old")]
#[path = "adapter_old.rs"]
mod adapter;
#[cfg(feature = "current")]
mod attribution;

entrypoint!(process);

pub const ITERATIONS: u64 = 100;
const SCALE: u64 = 1_000_000_000;

fn workload(operation: u8, index: u64) -> u64 {
    #[cfg(feature = "current")]
    if operation >= 200 {
        return attribution::workload(operation, index);
    }
    match operation {
        0 => black_box(index),
        100 => {
            let a = black_box(1_000_000 + index);
            let b = black_box(997);
            let denominator = black_box(1_003);
            adapter::mul_div_floor(a, b, denominator)
        }
        101 => {
            let a = black_box(1_000_000 + index);
            let b = black_box(997);
            let denominator = black_box(1_003);
            adapter::mul_div_ceil(a, b, denominator)
        }
        130 | 131 => {
            let a = black_box(1_000_000 + index);
            let b = black_box(997);
            let denominator = black_box(1_003);
            a ^ b ^ denominator
        }
        102 => {
            let value = black_box(u128::from(1_000_000 + index) * 1_000_003);
            adapter::isqrt(value)
        }
        132 => {
            let value = black_box(u128::from(1_000_000 + index) * 1_000_003);
            value as u64
        }
        103 => {
            let value = black_box(2_000_000_000 + index);
            adapter::sqrt_floor(value, SCALE)
        }
        104 => {
            let value = black_box(2_000_000_000 + index);
            adapter::sqrt_ceil(value, SCALE)
        }
        133 | 134 => {
            let value = black_box(2_000_000_000 + index);
            value
        }
        105 => {
            let value = black_box(500_000_000 + index) as i64;
            adapter::exp2_lower(value, SCALE)
        }
        106 => {
            let value = black_box(500_000_000 + index) as i64;
            adapter::exp2_upper(value, SCALE)
        }
        135 | 136 => {
            let value = black_box(500_000_000 + index) as i64;
            value as u64
        }
        107 => {
            let value = black_box(2 + index % 2);
            adapter::log2_lower(value, 1)
        }
        108 => {
            let value = black_box(2 + index % 2);
            adapter::log2_upper(value, 1)
        }
        137 | 138 => {
            let value = black_box(2 + index % 2);
            value
        }
        109 => {
            let base = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            adapter::pow_lower(base, exponent, SCALE)
        }
        110 => {
            let base = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            adapter::pow_upper(base, exponent, SCALE)
        }
        139 | 140 => {
            let base = black_box(2_000_000_000 + index);
            let exponent = black_box(500_000_000);
            base ^ exponent
        }
        111 => {
            let base = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            adapter::powi_lower(base, exponent, SCALE)
        }
        112 => {
            let base = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            adapter::powi_upper(base, exponent, SCALE)
        }
        141 | 142 => {
            let base = black_box(1_000_100_000 + index);
            let exponent = black_box(10 + index);
            base ^ exponent
        }
        113 => {
            let rate = black_box(70_000_000);
            let periods = black_box(63_072_000);
            let elapsed = black_box(63_072_000 + index);
            adapter::compound_lower(rate, periods, elapsed, SCALE)
        }
        114 => {
            let rate = black_box(70_000_000);
            let periods = black_box(63_072_000);
            let elapsed = black_box(63_072_000 + index);
            adapter::compound_upper(rate, periods, elapsed, SCALE)
        }
        143 | 144 => {
            let rate = black_box(70_000_000);
            let periods = black_box(63_072_000);
            let elapsed = black_box(63_072_000 + index);
            rate ^ periods ^ elapsed
        }
        115 => {
            let amount = black_box(1_000_000 + index);
            let fee_bps = black_box(30_u16);
            adapter::net_of_fee(amount, fee_bps)
        }
        145 => {
            let amount = black_box(1_000_000 + index);
            let fee_bps = black_box(30_u16);
            amount ^ u64::from(fee_bps)
        }
        116 => {
            let reserve_in = black_box(50_000_000 + index);
            let reserve_out = black_box(80_000_000_000);
            let amount = black_box(1_000_000 + index);
            let fee_bps = black_box(30_u16);
            adapter::quote_exact_in(reserve_in, reserve_out, amount, fee_bps)
        }
        117 => {
            let reserve_in = black_box(50_000_000 + index);
            let reserve_out = black_box(80_000_000_000);
            let amount = black_box(1_000_000_000 + index);
            let fee_bps = black_box(30_u16);
            adapter::quote_exact_out(reserve_in, reserve_out, amount, fee_bps)
        }
        146 | 147 => {
            let reserve_in = black_box(50_000_000 + index);
            let reserve_out = black_box(80_000_000_000);
            let amount = black_box(if operation == 146 {
                1_000_000 + index
            } else {
                1_000_000_000 + index
            });
            let fee_bps = black_box(30_u16);
            reserve_in ^ reserve_out ^ amount ^ u64::from(fee_bps)
        }
        118 => {
            let a = black_box(4_000_000 + index);
            let b = black_box(9_000_000);
            adapter::initial_lp_shares(a, b)
        }
        148 => {
            let a = black_box(4_000_000 + index);
            let b = black_box(9_000_000);
            a ^ b
        }
        119 => {
            let borrowed = black_box(70_000_000 + index);
            let supplied = black_box(100_000_000);
            adapter::utilization_bps(borrowed, supplied)
        }
        149 => {
            let borrowed = black_box(70_000_000 + index);
            let supplied = black_box(100_000_000);
            borrowed ^ supplied
        }
        120 => {
            let utilization = black_box(8_001 + index % 100);
            let base = black_box(100);
            let before = black_box(500);
            let after = black_box(2_000);
            let kink = black_box(8_000);
            adapter::borrow_rate_bps(utilization, base, before, after, kink)
        }
        150 => {
            let utilization = black_box(8_001 + index % 100);
            let base = black_box(100);
            let before = black_box(500);
            let after = black_box(2_000);
            let kink = black_box(8_000);
            utilization ^ base ^ before ^ after ^ kink
        }
        121 => {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            adapter::reward_index_lower(index_value, reward, stake, SCALE)
        }
        122 => {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            adapter::reward_index_upper(index_value, reward, stake, SCALE)
        }
        151 | 152 => {
            let index_value = black_box(1_250_000_000);
            let reward = black_box(1_000_000 + index);
            let stake = black_box(100_000_000);
            index_value ^ reward ^ stake
        }
        123 => {
            let staked = black_box(100_000_000 + index);
            let now = black_box(1_300_000_000);
            let snapshot = black_box(1_250_000_000);
            adapter::rewards_owed(staked, now, snapshot, SCALE)
        }
        153 => {
            let staked = black_box(100_000_000 + index);
            let now = black_box(1_300_000_000);
            let snapshot = black_box(1_250_000_000);
            staked ^ now ^ snapshot
        }
        124 => {
            let price = black_box(20_000_000 + index as i64);
            let confidence = black_box(100_000);
            let exponent = black_box(-6);
            adapter::oracle_bounds(price, confidence, exponent, SCALE)
        }
        154 => {
            let price = black_box(20_000_000 + index as i64);
            let confidence = black_box(100_000_u64);
            let exponent = black_box(-6_i32);
            price as u64 ^ confidence ^ u64::from(exponent.unsigned_abs())
        }
        158 | 159 => {
            let price = black_box(20_000_000 + index as i64);
            let confidence = black_box(100_000);
            let exponent = black_box(-6);
            if operation == 158 {
                adapter::oracle_lower(price, confidence, exponent, SCALE)
            } else {
                adapter::oracle_upper(price, confidence, exponent, SCALE)
            }
        }
        125 => {
            let total = black_box(1_000_000_000);
            let start = black_box(1_000);
            let cliff = black_box(1_500);
            let duration = black_box(10_000);
            let now = black_box(2_000 + index);
            adapter::vested(total, start, cliff, duration, now)
        }
        155 => {
            let total = black_box(1_000_000_000);
            let start = black_box(1_000);
            let cliff = black_box(1_500);
            let duration = black_box(10_000);
            let now = black_box(2_000 + index);
            total ^ start ^ cliff ^ duration ^ now
        }
        126 => {
            let from = black_box(1_000_000);
            let to = black_box(2_000_000);
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            adapter::interp_floor(from, to, elapsed, duration)
        }
        127 => {
            let from = black_box(2_000_000);
            let to = black_box(1_000_000);
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            adapter::interp_ceil(from, to, elapsed, duration)
        }
        156 | 157 => {
            let from = black_box(if operation == 156 {
                1_000_000
            } else {
                2_000_000
            });
            let to = black_box(if operation == 156 {
                2_000_000
            } else {
                1_000_000
            });
            let elapsed = black_box(1_000 + index);
            let duration = black_box(10_000);
            from ^ to ^ elapsed ^ duration
        }
        _ => 0,
    }
}

fn encode_pair(xor: u64, sum: u64) -> [u8; 16] {
    let mut bytes = [0_u8; 16];
    bytes[..8].copy_from_slice(&xor.to_le_bytes());
    bytes[8..].copy_from_slice(&sum.to_le_bytes());
    bytes
}

pub fn process(_program_id: &Pubkey, _accounts: &[AccountInfo], data: &[u8]) -> ProgramResult {
    let operation = data.first().copied().unwrap_or(0);
    if data.get(1) == Some(&1) {
        let mut index_bytes = [0_u8; 8];
        if let Some(bytes) = data.get(2..10) {
            index_bytes.copy_from_slice(bytes);
        }
        set_return_data(&workload(operation, u64::from_le_bytes(index_bytes)).to_le_bytes());
        return Ok(());
    }

    let mut xor = 0_u64;
    let mut sum = 0_u64;
    for raw_index in 0..ITERATIONS {
        let value = workload(operation, black_box(raw_index));
        xor ^= value;
        sum = sum.wrapping_add(value);
    }
    set_return_data(&encode_pair(xor, sum));
    Ok(())
}
