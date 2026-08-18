//! CU measurement via litesvm: send one transaction per op, read
//! compute_units_consumed, subtract the baseline instruction, divide by the
//! iteration count. Prints a markdown table.

use litesvm::LiteSVM;
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::sync::atomic::{AtomicU64, Ordering};

/// The complete public benchmark set. The manifest must contain this exact
/// set; it orders that set lexicographically by public label. Controls are not
/// benchmark rows.
const PUBLIC_OPS: &[(u8, &str)] = &[
    (100, "mul_div_floor"),
    (101, "mul_div_ceil"),
    (102, "isqrt"),
    (103, "sqrt_floor"),
    (104, "sqrt_ceil"),
    (105, "exp2_lower"),
    (129, "exp2_bounds"),
    (106, "exp2_upper"),
    (107, "log2_lower"),
    (160, "log2_bounds"),
    (108, "log2_upper"),
    (109, "pow_lower"),
    (161, "pow_bounds"),
    (110, "pow_upper"),
    (111, "powi_lower"),
    (112, "powi_upper"),
    (113, "compound_lower"),
    (162, "compound_bounds"),
    (114, "compound_upper"),
    (115, "defi::fee::net_of_fee"),
    (116, "defi::amm::quote_exact_in"),
    (117, "defi::amm::quote_exact_out"),
    (118, "defi::amm::initial_lp_shares_floor"),
    (119, "defi::lending::utilization_bps"),
    (120, "defi::lending::borrow_rate_bps"),
    (121, "defi::staking::reward_index_accrue_lower"),
    (163, "defi::staking::reward_index_accrue"),
    (122, "defi::staking::reward_index_accrue_upper"),
    (123, "defi::staking::rewards_owed_floor"),
    (124, "defi::oracle::price_bounds_scaled"),
    (125, "defi::schedule::vested_floor"),
    (126, "defi::schedule::linear_interp_floor"),
    (127, "defi::schedule::linear_interp_ceil"),
    (128, "mul_div_floor_wide"),
];

static TRANSACTION_NONCE: AtomicU64 = AtomicU64::new(0);
const WORKLOAD_MANIFEST: &str = include_str!("../../../verification/sbf-workloads.txt");

const ITERATIONS: u64 = 100;

#[derive(Clone, Copy)]
struct Workload {
    target: u8,
    label: &'static str,
    control: u8,
    /// Frozen pre-rewrite CU/op reference used only for comparison.
    reference: u64,
}

fn iters(_operation: u8) -> u64 {
    ITERATIONS
}

fn matched_control(target: u8) -> u8 {
    target
        .checked_add(30)
        .expect("public target ID has a matched control ID")
}

fn parse_canonical_u8(value: &str, context: &str) -> u8 {
    assert!(
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
        "{context} must be an unsigned decimal integer"
    );
    let parsed = value
        .parse::<u8>()
        .unwrap_or_else(|_| panic!("{context} is outside the u8 range"));
    assert_eq!(parsed.to_string(), value, "{context} is not canonical");
    parsed
}

fn parse_canonical_u64(value: &str, context: &str) -> u64 {
    assert!(
        !value.is_empty() && value.bytes().all(|byte| byte.is_ascii_digit()),
        "{context} must be an unsigned decimal integer"
    );
    let parsed = value
        .parse::<u64>()
        .unwrap_or_else(|_| panic!("{context} is outside the u64 range"));
    assert_eq!(parsed.to_string(), value, "{context} is not canonical");
    parsed
}

/// Parse the single authoritative workload artifact. It intentionally has no
/// comments or blank lines: every line is exactly
/// `<target-id> <public-label> <control-id> <historical-cu-per-op>`.
fn workloads() -> Vec<Workload> {
    assert!(
        !WORKLOAD_MANIFEST.is_empty(),
        "workload manifest must not be empty"
    );

    let rows: Vec<Workload> = WORKLOAD_MANIFEST
        .lines()
        .enumerate()
        .map(|(index, line)| {
            let line_number = index + 1;
            assert!(
                !line.is_empty(),
                "workload manifest line {line_number} must not be blank"
            );
            let fields: Vec<&str> = line.split(' ').collect();
            assert_eq!(
                fields.len(),
                4,
                "workload manifest line {line_number} must contain exactly four single-space-separated fields"
            );
            assert!(
                fields.iter().all(|field| !field.is_empty()),
                "workload manifest line {line_number} contains repeated or edge whitespace"
            );

            let target = parse_canonical_u8(fields[0], &format!("line {line_number} target ID"));
            let control = parse_canonical_u8(fields[2], &format!("line {line_number} control ID"));
            let reference =
                parse_canonical_u64(fields[3], &format!("line {line_number} CU reference"));
            assert!(
                reference > 0,
                "workload manifest line {line_number} CU reference must be positive"
            );
            Workload {
                target,
                label: fields[1],
                control,
                reference,
            }
        })
        .collect();

    assert_eq!(
        rows.len(),
        PUBLIC_OPS.len(),
        "workload manifest must contain exactly the declared benchmark rows"
    );
    for window in rows.windows(2) {
        assert_eq!(
            window[0].label.cmp(window[1].label),
            std::cmp::Ordering::Less,
            "workload manifest labels must be strictly lexicographically sorted"
        );
    }
    for &(expected_target, expected_label) in PUBLIC_OPS {
        let matching_rows = rows
            .iter()
            .filter(|row| row.target == expected_target && row.label == expected_label)
            .count();
        assert_eq!(
            matching_rows, 1,
            "workload manifest must contain exactly one row for {expected_label} ({expected_target})"
        );
    }
    for row in &rows {
        assert_eq!(
            row.control,
            matched_control(row.target),
            "workload manifest control mismatch for {} ({})",
            row.label,
            row.target
        );
    }
    rows
}

fn run(svm: &mut LiteSVM, payer: &Keypair, program_id: Pubkey, op: u8) -> u64 {
    let data = {
        let mut d = vec![op];
        d.extend_from_slice(&0xDEAD_BEEF_CAFE_F00Du64.to_le_bytes());
        d.extend_from_slice(
            &TRANSACTION_NONCE
                .fetch_add(1, Ordering::Relaxed)
                .to_le_bytes(),
        );
        d
    };
    let ix = Instruction {
        program_id,
        accounts: vec![],
        data,
    };
    let budget = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
    let msg = Message::new(&[budget, ix], Some(&payer.pubkey()));
    let tx = Transaction::new(&[payer], msg, svm.latest_blockhash());
    let meta = svm
        .send_transaction(tx)
        .unwrap_or_else(|e| panic!("op {op} failed: {e:?}"));
    meta.compute_units_consumed
}

fn main() {
    let workloads = workloads();
    let so_path = std::env::args()
        .nth(1)
        .unwrap_or_else(|| "target/deploy/svm_math_bench_program.so".into());
    let bytes = std::fs::read(&so_path).expect("program .so not found — run cargo build-sbf first");

    let mut svm = LiteSVM::new();
    let program_id = Pubkey::new_unique();
    svm.add_program(program_id, &bytes);
    let payer = Keypair::new();
    svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();

    // Correctness gate: op 30 checks compiler-native u128 division on-chain
    // (q·d + r == n, r < d) before any measurement is printed.
    run(&mut svm, &payer, program_id, 30);

    let repetitions = match std::env::var("SVM_MATH_REPETITIONS") {
        Ok(value) => value
            .parse::<usize>()
            .ok()
            .filter(|count| *count >= 3)
            .unwrap_or_else(|| panic!("SVM_MATH_REPETITIONS must be an integer of at least 3")),
        Err(std::env::VarError::NotPresent) => 3,
        Err(_) => panic!("SVM_MATH_REPETITIONS must be valid UTF-8"),
    };
    let baseline100 = run(&mut svm, &payer, program_id, 0);
    println!("baseline (tx + empty 100-loop): {baseline100} CU\n");
    println!("repetitions: {repetitions}\n");
    println!("| operation | CU/op min | CU/op max | spread | historical ref | delta |");
    println!("|---|---:|---:|---:|---:|---:|");
    for workload in workloads {
        let n = iters(workload.target);
        let mut samples = Vec::with_capacity(repetitions);
        for _ in 0..repetitions {
            let base = run(&mut svm, &payer, program_id, workload.control);
            let total = run(&mut svm, &payer, program_id, workload.target);
            let delta = total.checked_sub(base).unwrap_or_else(|| {
                panic!(
                    "{} target total {total} CU is below matched control {} CU",
                    workload.label, base
                )
            });
            samples.push(delta.checked_add(n - 1).expect("CU/op rounding overflow") / n);
        }
        let min = *samples.iter().min().expect("positive repetition count");
        let max = *samples.iter().max().expect("positive repetition count");
        let spread = max - min;
        assert!(
            spread <= 2,
            "{} spread {spread} CU/op exceeds 2",
            workload.label
        );
        let delta = i128::from(max) - i128::from(workload.reference);
        println!(
            "| {} | {min} | {max} | {spread} | {} | {delta:+} |",
            workload.label, workload.reference
        );
    }
}
