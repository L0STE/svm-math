use litesvm::LiteSVM;
use rug::{float::Round, ops::Pow, Float};
use solana_sdk::{
    compute_budget::ComputeBudgetInstruction,
    instruction::Instruction,
    message::Message,
    pubkey::Pubkey,
    signature::{Keypair, Signer},
    transaction::Transaction,
};
use std::{
    path::Path,
    sync::atomic::{AtomicU64, Ordering},
};

const ITERATIONS: u64 = 100;
const REPETITIONS: usize = 3;
const WORKLOADS: &str = include_str!("../../workloads.tsv");

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Boundary {
    Kernel,
    Consumer,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Parity {
    Exact,
    Directed,
}

#[derive(Clone, Copy, Debug)]
struct Workload {
    target: u8,
    label: &'static str,
    control: u8,
    boundary: Boundary,
    parity: Parity,
}

struct Vm {
    svm: LiteSVM,
    payer: Keypair,
    program_id: Pubkey,
    nonce: AtomicU64,
}

impl Vm {
    fn new(path: &Path) -> Self {
        let bytes = std::fs::read(path)
            .unwrap_or_else(|error| panic!("failed to read {}: {error}", path.display()));
        let mut svm = LiteSVM::new();
        let program_id = Pubkey::new_unique();
        svm.add_program(program_id, &bytes);
        let payer = Keypair::new();
        svm.airdrop(&payer.pubkey(), 10_000_000_000).unwrap();
        Self {
            svm,
            payer,
            program_id,
            nonce: AtomicU64::new(0),
        }
    }

    fn run(&mut self, mut data: Vec<u8>) -> (u64, Vec<u8>) {
        data.extend_from_slice(&self.nonce.fetch_add(1, Ordering::Relaxed).to_le_bytes());
        let instruction = Instruction {
            program_id: self.program_id,
            accounts: vec![],
            data,
        };
        let budget = ComputeBudgetInstruction::set_compute_unit_limit(1_400_000);
        let message = Message::new(&[budget, instruction], Some(&self.payer.pubkey()));
        let transaction = Transaction::new(&[&self.payer], message, self.svm.latest_blockhash());
        let metadata = self
            .svm
            .send_transaction(transaction)
            .unwrap_or_else(|error| panic!("SBF transaction failed: {error:?}"));
        (metadata.compute_units_consumed, metadata.return_data.data)
    }

    fn measure(&mut self, operation: u8) -> (u64, Vec<u8>) {
        self.run(vec![operation, 0])
    }

    fn probe(&mut self, operation: u8, index: u64) -> u64 {
        let mut data = vec![operation, 1];
        data.extend_from_slice(&index.to_le_bytes());
        let (_, output) = self.run(data);
        assert_eq!(output.len(), 8, "probe return must be one u64");
        u64::from_le_bytes(output.try_into().unwrap())
    }
}

fn workloads() -> Vec<Workload> {
    let rows: Vec<_> = WORKLOADS
        .lines()
        .map(|line| {
            let fields: Vec<_> = line.split('\t').collect();
            assert_eq!(fields.len(), 5, "invalid workload row: {line}");
            Workload {
                target: fields[0].parse().unwrap(),
                label: fields[1],
                control: fields[2].parse().unwrap(),
                boundary: match fields[3] {
                    "kernel" => Boundary::Kernel,
                    "consumer" => Boundary::Consumer,
                    other => panic!("invalid boundary {other}"),
                },
                parity: match fields[4] {
                    "exact" => Parity::Exact,
                    "directed" => Parity::Directed,
                    other => panic!("invalid parity {other}"),
                },
            }
        })
        .collect();
    assert_eq!(rows.len(), 28);
    rows
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn per_op(target: u64, control: u64) -> u64 {
    target
        .checked_sub(control)
        .expect("target CU must not be below its matched control")
        .checked_add(ITERATIONS - 1)
        .unwrap()
        / ITERATIONS
}

const ORACLE_PRECISION: u32 = 512;
const SCALE: u64 = 1_000_000_000;

fn ratio_u64(numerator: u64, denominator: u64, round: Round) -> Float {
    let numerator = Float::with_val(ORACLE_PRECISION, numerator);
    let denominator = Float::with_val(ORACLE_PRECISION, denominator);
    Float::with_val_round(ORACLE_PRECISION, &numerator / &denominator, round).0
}

fn scaled(lower: &Float, upper: &Float, scale: u64) -> (Float, Float) {
    (
        Float::with_val_round(ORACLE_PRECISION, lower * scale, Round::Down).0,
        Float::with_val_round(ORACLE_PRECISION, upper * scale, Round::Up).0,
    )
}

fn truth_pair(label: &str, index: u64) -> (Float, Float) {
    match label {
        "sqrt" => {
            let value = 2_000_000_000 + index;
            let lower = ratio_u64(value, SCALE, Round::Down);
            let upper = ratio_u64(value, SCALE, Round::Up);
            let lower = Float::with_val_round(ORACLE_PRECISION, lower.sqrt_ref(), Round::Down).0;
            let upper = Float::with_val_round(ORACLE_PRECISION, upper.sqrt_ref(), Round::Up).0;
            scaled(&lower, &upper, SCALE)
        }
        "exp2" => {
            let value = 500_000_000 + index;
            let lower = ratio_u64(value, SCALE, Round::Down);
            let upper = ratio_u64(value, SCALE, Round::Up);
            let lower = Float::with_val_round(ORACLE_PRECISION, lower.exp2_ref(), Round::Down).0;
            let upper = Float::with_val_round(ORACLE_PRECISION, upper.exp2_ref(), Round::Up).0;
            scaled(&lower, &upper, SCALE)
        }
        "log2" => {
            let value = 2 + index % 2;
            let lower = Float::with_val_round(
                ORACLE_PRECISION,
                Float::with_val(ORACLE_PRECISION, value).log2_ref(),
                Round::Down,
            )
            .0;
            let upper = Float::with_val_round(
                ORACLE_PRECISION,
                Float::with_val(ORACLE_PRECISION, value).log2_ref(),
                Round::Up,
            )
            .0;
            (lower, upper)
        }
        "pow" => {
            let base = 2_000_000_000 + index;
            let base_lower = ratio_u64(base, SCALE, Round::Down);
            let base_upper = ratio_u64(base, SCALE, Round::Up);
            let exponent_lower = ratio_u64(500_000_000, SCALE, Round::Down);
            let exponent_upper = ratio_u64(500_000_000, SCALE, Round::Up);
            let lower = Float::with_val_round(
                ORACLE_PRECISION,
                (&base_lower).pow(&exponent_lower),
                Round::Down,
            )
            .0;
            let upper = Float::with_val_round(
                ORACLE_PRECISION,
                (&base_upper).pow(&exponent_upper),
                Round::Up,
            )
            .0;
            scaled(&lower, &upper, SCALE)
        }
        "powi" => {
            let base = 1_000_100_000 + index;
            let exponent = 10 + index;
            let base_lower = ratio_u64(base, SCALE, Round::Down);
            let base_upper = ratio_u64(base, SCALE, Round::Up);
            let lower = Float::with_val_round(
                ORACLE_PRECISION,
                (&base_lower).pow(exponent),
                Round::Down,
            )
            .0;
            let upper = Float::with_val_round(
                ORACLE_PRECISION,
                (&base_upper).pow(exponent),
                Round::Up,
            )
            .0;
            scaled(&lower, &upper, SCALE)
        }
        "compound" => {
            let periods = 63_072_000;
            let elapsed = periods + index;
            let denominator = u128::from(periods) * u128::from(SCALE);
            let rate = Float::with_val(ORACLE_PRECISION, 70_000_000);
            let denominator = Float::with_val(ORACLE_PRECISION, denominator);
            let x_lower =
                Float::with_val_round(ORACLE_PRECISION, &rate / &denominator, Round::Down).0;
            let x_upper =
                Float::with_val_round(ORACLE_PRECISION, &rate / &denominator, Round::Up).0;
            let one = Float::with_val(ORACLE_PRECISION, 1);
            let base_lower =
                Float::with_val_round(ORACLE_PRECISION, &one + &x_lower, Round::Down).0;
            let base_upper =
                Float::with_val_round(ORACLE_PRECISION, &one + &x_upper, Round::Up).0;
            let lower = Float::with_val_round(
                ORACLE_PRECISION,
                (&base_lower).pow(elapsed),
                Round::Down,
            )
            .0;
            let upper = Float::with_val_round(
                ORACLE_PRECISION,
                (&base_upper).pow(elapsed),
                Round::Up,
            )
            .0;
            scaled(&lower, &upper, SCALE)
        }
        "reward_index_accrue" => {
            let reward = 1_000_000 + index;
            let index = Float::with_val(ORACLE_PRECISION, 1_250_000_000);
            let numerator = Float::with_val(ORACLE_PRECISION, reward) * SCALE;
            let denominator = Float::with_val(ORACLE_PRECISION, 100_000_000);
            let delta_lower = Float::with_val_round(
                ORACLE_PRECISION,
                &numerator / &denominator,
                Round::Down,
            )
            .0;
            let delta_upper = Float::with_val_round(
                ORACLE_PRECISION,
                &numerator / &denominator,
                Round::Up,
            )
            .0;
            (
                Float::with_val_round(ORACLE_PRECISION, &index + delta_lower, Round::Down).0,
                Float::with_val_round(ORACLE_PRECISION, &index + delta_upper, Round::Up).0,
            )
        }
        "oracle" => {
            let price = 20_000_000_i128 + i128::from(index);
            let confidence = 100_000_i128;
            let factor = 1_000_i128;
            (
                Float::with_val(ORACLE_PRECISION, (price - confidence) * factor),
                Float::with_val(ORACLE_PRECISION, (price + confidence) * factor),
            )
        }
        _ => unreachable!("unknown directed workload {label}"),
    }
}

fn assert_encloses(revision: &str, label: &str, index: u64, lower: u64, upper: u64) {
    let (truth_lower, truth_upper) = truth_pair(label, index);
    assert!(lower <= upper, "{revision} {label} enclosure inverted at {index}");
    assert!(
        Float::with_val(ORACLE_PRECISION, lower) <= truth_lower
            && truth_upper <= Float::with_val(ORACLE_PRECISION, upper),
        "{revision} {label} failed MPFR enclosure at {index}: [{lower}, {upper}] vs [{truth_lower}, {truth_upper}]"
    );
}

fn main() {
    let mut args = std::env::args().skip(1);
    let old_path = args.next().expect("old .so path");
    let current_path = args.next().expect("current .so path");
    let harness_hash = args.next().expect("shared harness sha256");
    let old_revision = args.next().expect("old revision");
    let current_revision = args.next().expect("current revision");
    let current_diff_hash = args.next().expect("current diff hash");
    assert!(args.next().is_none(), "unexpected extra argument");

    println!("old_revision={old_revision}");
    println!("current_revision={current_revision}");
    println!("current_diff_hash={current_diff_hash}");
    println!("shared_harness_sha256={harness_hash}");
    println!("iterations={ITERATIONS}");
    println!("repetitions={REPETITIONS}");

    let mut old = Vm::new(Path::new(&old_path));
    let mut current = Vm::new(Path::new(&current_path));
    let rows = workloads();

    for row in &rows {
        if row.parity == Parity::Exact {
            for index in 0..ITERATIONS {
                let old_output = old.probe(row.target, index);
                let current_output = current.probe(row.target, index);
                assert_eq!(
                    old_output, current_output,
                    "exact output mismatch for {} at index {index}",
                    row.label
                );
            }
        }
    }

    for &(lower, upper, label) in &[
        (103, 104, "sqrt"),
        (105, 106, "exp2"),
        (107, 108, "log2"),
        (109, 110, "pow"),
        (111, 112, "powi"),
        (113, 114, "compound"),
        (121, 122, "reward_index_accrue"),
        (158, 159, "oracle"),
    ] {
        for index in 0..ITERATIONS {
            let old_lower = old.probe(lower, index);
            let old_upper = old.probe(upper, index);
            let current_lower = current.probe(lower, index);
            let current_upper = current.probe(upper, index);
            assert_encloses("old", label, index, old_lower, old_upper);
            assert_encloses(
                "current",
                label,
                index,
                current_lower,
                current_upper,
            );
        }
    }

    for index in 0..ITERATIONS {
        let expected = (u128::from(100_000_000 + index) * 50_000_000 / u128::from(SCALE)) as u64;
        for (revision, output) in [
            ("old", old.probe(123, index)),
            ("current", current.probe(123, index)),
        ] {
            assert!(
                output <= expected && expected - output <= 1,
                "{revision} rewards_owed_floor violated conservative one-unit contract at {index}: {output} vs {expected}"
            );
        }
    }

    println!("exact_output_probes=pass");
    println!("directed_mpfr_or_contract_probes=pass");
    println!();
    println!("| boundary | operation | old CU/op | current CU/op | consumer delta | kernel-only delta | output relation | old checksum | current checksum |");
    println!("|---|---|---:|---:|---:|---:|---|---|---|");

    for row in rows {
        let mut old_samples = Vec::with_capacity(REPETITIONS);
        let mut current_samples = Vec::with_capacity(REPETITIONS);
        let mut old_output = Vec::new();
        let mut current_output = Vec::new();
        for _ in 0..REPETITIONS {
            let (old_control, _) = old.measure(row.control);
            let (old_target, observed_old) = old.measure(row.target);
            let (current_control, _) = current.measure(row.control);
            let (current_target, observed_current) = current.measure(row.target);
            old_samples.push(per_op(old_target, old_control));
            current_samples.push(per_op(current_target, current_control));
            if old_output.is_empty() {
                old_output = observed_old;
                current_output = observed_current;
            } else {
                assert_eq!(
                    old_output, observed_old,
                    "unstable old output for {}",
                    row.label
                );
                assert_eq!(
                    current_output, observed_current,
                    "unstable current output for {}",
                    row.label
                );
            }
        }
        let old_cu = *old_samples.iter().max().unwrap();
        let current_cu = *current_samples.iter().max().unwrap();
        assert!(old_samples.iter().max().unwrap() - old_samples.iter().min().unwrap() <= 2);
        assert!(current_samples.iter().max().unwrap() - current_samples.iter().min().unwrap() <= 2);
        let delta = i128::from(current_cu) - i128::from(old_cu);
        let relation = if old_output == current_output {
            "equal"
        } else {
            "different-directed"
        };
        if row.parity == Parity::Exact {
            assert_eq!(
                relation, "equal",
                "exact checksum mismatch for {}",
                row.label
            );
        }
        let kernel_delta = match row.boundary {
            Boundary::Kernel => format!("{delta:+}"),
            Boundary::Consumer => "n/a".to_owned(),
        };
        println!(
            "| {:?} | {} | {old_cu} | {current_cu} | {delta:+} | {kernel_delta} | {relation} | {} | {} |",
            row.boundary,
            row.label,
            hex(&old_output),
            hex(&current_output),
        );
    }
}
