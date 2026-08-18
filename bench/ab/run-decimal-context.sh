#!/usr/bin/env bash
set -euo pipefail

: "${SBF_BUILDER:?set SBF_BUILDER to the cargo-build-sbf executable}"
builder=$(command -v "$SBF_BUILDER")
repo=$(git rev-parse --show-toplevel)
output_dir=${1:-"$repo/target/ab-decimal-context"}
mkdir -p "$output_dir"
output_dir=$(cd "$output_dir" && pwd)
temporary_root=$(mktemp -d)
baseline="$temporary_root/baseline"
prototype="$temporary_root/prototype"
cleanup() { rm -rf "$temporary_root"; }
trap cleanup EXIT

rsync -a --exclude .git/ --exclude target/ --exclude bench/target/ --exclude bench/ab/target/ "$repo/" "$baseline/"
rsync -a --exclude .git/ --exclude target/ --exclude bench/target/ --exclude bench/ab/target/ "$repo/" "$prototype/"
git -C "$baseline" apply "$repo/bench/ab/decimal-baseline.patch"

baseline_harness=$(shasum -a 256 "$baseline/bench/ab/program/src/lib.rs" | awk '{print $1}')
prototype_harness=$(shasum -a 256 "$prototype/bench/ab/program/src/lib.rs" | awk '{print $1}')
test "$baseline_harness" = "$prototype_harness"
{
    printf 'builder_path=%s\n' "$builder"
    printf 'builder_sha256=%s\n' "$(shasum -a 256 "$builder" | awk '{print $1}')"
    "$builder" --version
    printf 'shared_harness_sha256=%s\n' "$prototype_harness"
    printf 'baseline_patch_sha256=%s\n' "$(shasum -a 256 "$repo/bench/ab/decimal-baseline.patch" | awk '{print $1}')"
} > "$output_dir/provenance.txt"

(cd "$baseline/bench/ab/program" && "$builder" --no-default-features --features current) 2>&1 | tee "$output_dir/baseline-build.txt"
(cd "$prototype/bench/ab/program" && "$builder" --no-default-features --features current) 2>&1 | tee "$output_dir/prototype-build.txt"
cp "$baseline/bench/ab/target/deploy/svm_math_ab_program.so" "$output_dir/baseline.so"
cp "$prototype/bench/ab/target/deploy/svm_math_ab_program.so" "$output_dir/prototype.so"
{
    stat -f 'baseline_so_bytes=%z' "$output_dir/baseline.so"
    stat -f 'prototype_so_bytes=%z' "$output_dir/prototype.so"
} | tee "$output_dir/sizes.txt"

for variant in baseline prototype; do
    cargo +1.89.0 run --release --manifest-path "$repo/bench/ab/Cargo.toml" \
        --package svm-math-ab-runner --bin attribution -- \
        "$output_dir/$variant.so" "$prototype_harness" "$variant" \
        | tee "$output_dir/$variant.md"
done
