use litesvm::LiteSVM;
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

struct Vm {
    svm: LiteSVM,
    payer: Keypair,
    program_id: Pubkey,
    nonce: AtomicU64,
}

impl Vm {
    fn new(path: &Path) -> Self {
        let bytes = std::fs::read(path).unwrap();
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
        let metadata = self.svm.send_transaction(transaction).unwrap();
        (metadata.compute_units_consumed, metadata.return_data.data)
    }

    fn measure(&mut self, operation: u8) -> (u64, Vec<u8>) {
        self.run(vec![operation, 0])
    }

    fn probe(&mut self, operation: u8, index: u64) -> u64 {
        let mut data = vec![operation, 1];
        data.extend_from_slice(&index.to_le_bytes());
        let (_, output) = self.run(data);
        u64::from_le_bytes(output.try_into().unwrap())
    }
}

fn per_op(target: u64, control: u64) -> u64 {
    (target - control).div_ceil(ITERATIONS)
}

fn sample(vm: &mut Vm, target: u8, control: u8) -> (u64, Vec<u8>) {
    let mut samples = Vec::new();
    let mut output = Vec::new();
    for _ in 0..REPETITIONS {
        let (control_cu, _) = vm.measure(control);
        let (target_cu, observed) = vm.measure(target);
        samples.push(per_op(target_cu, control_cu));
        if output.is_empty() {
            output = observed;
        } else {
            assert_eq!(output, observed);
        }
    }
    assert!(samples.iter().max().unwrap() - samples.iter().min().unwrap() <= 2);
    (*samples.iter().max().unwrap(), output)
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|byte| format!("{byte:02x}")).collect()
}

fn main() {
    let mut args = std::env::args().skip(1);
    let artifact = args.next().expect("current .so path");
    let harness_hash = args.next().expect("shared harness sha256");
    let state_hash = args.next().expect("current state hash");
    assert!(args.next().is_none());
    println!("artifact={artifact}");
    println!("current_state_hash={state_hash}");
    println!("shared_harness_sha256={harness_hash}");
    println!("iterations={ITERATIONS}");
    println!("repetitions={REPETITIONS}");

    let mut vm = Vm::new(Path::new(&artifact));
    let pairs = [
        ("widening pair-word vs native-u128", 200, 201, 230, true),
        ("decimal entry old-wide vs decomposed", 202, 203, 232, true),
        ("exp2 exit lower Q61-round vs direct-Q63", 205, 206, 235, false),
        ("exp2 exit upper Q61-round vs direct-Q63", 213, 214, 243, false),
        ("exp2 full lower old-exit vs direct-Q63", 207, 208, 237, false),
        ("exp2 full upper old-exit vs direct-Q63", 215, 216, 245, false),
        ("powi lower repeated divider vs FixedDivisor", 209, 210, 239, true),
        ("powi upper repeated divider vs FixedDivisor", 211, 212, 241, true),
    ];
    for &(label, before, after, _, exact) in &pairs {
        for index in 0..ITERATIONS {
            let before_output = vm.probe(before, index);
            let after_output = vm.probe(after, index);
            if exact {
                assert_eq!(before_output, after_output, "{label} at {index}");
            }
        }
    }
    for index in 0..ITERATIONS {
        assert_eq!(vm.probe(228, index), vm.probe(229, index), "1e18 entry wide leg at {index}");
        assert_eq!(vm.probe(231, index), vm.probe(233, index), "1e18 normalization at {index}");
    }
    println!("exact_variant_probes=pass");
    println!();
    println!("| attribution boundary | variant pair | before CU/op | after CU/op | delta | output relation | before checksum | after checksum |");
    println!("|---|---|---:|---:|---:|---|---|---|");
    for (label, before, after, control, exact) in pairs {
        let (before_cu, before_output) = sample(&mut vm, before, control);
        let (after_cu, after_output) = sample(&mut vm, after, control);
        let relation = if before_output == after_output {
            "equal"
        } else if exact {
            panic!("exact checksum mismatch for {label}")
        } else {
            "different-directed"
        };
        let delta = i128::from(after_cu) - i128::from(before_cu);
        println!(
            "| isolated | {label} | {before_cu} | {after_cu} | {delta:+} | {relation} | {} | {} |",
            hex(&before_output),
            hex(&after_output),
        );
    }
    let (core_cu, checksum) = sample(&mut vm, 204, 234);
    println!(
        "| common kernel | exp2 mantissa core (no A/B) | n/a | {core_cu} | n/a | single-path | n/a | {} |",
        hex(&checksum)
    );
    for (label, target, control) in [
        ("1e18 exp2 lower", 217, 246),
        ("1e18 exp2 upper", 218, 246),
        ("1e18 log2 lower", 219, 247),
        ("1e18 log2 upper", 220, 247),
        ("1e18 pow lower", 221, 248),
        ("1e18 pow upper", 222, 248),
        ("1e18 powi lower", 223, 249),
        ("1e18 powi upper", 224, 249),
        ("1e18 compound lower", 225, 250),
        ("1e18 compound upper", 226, 250),
        ("1e18 whole/remainder only", 227, 251),
        ("1e18 wide entry runtime reciprocal", 228, 252),
        ("1e18 wide entry prepared metadata", 229, 252),
        ("1e18 normalize runtime reciprocal", 231, 253),
        ("1e18 normalize prepared metadata", 233, 253),
    ] {
        let (cu, checksum) = sample(&mut vm, target, control);
        println!(
            "| absolute | {label} | n/a | {cu} | n/a | single-path | n/a | {} |",
            hex(&checksum)
        );
    }
}
