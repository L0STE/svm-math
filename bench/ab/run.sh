#!/usr/bin/env bash
set -euo pipefail

old_revision=f589fc21559dcfac4a2335e7bc595e4bec27c361
repo=$(git rev-parse --show-toplevel)
test -f "$repo/bench/ab/program/src/lib.rs"
: "${SBF_BUILDER:?set SBF_BUILDER to the cargo-build-sbf executable used for both artifacts}"
builder=$(command -v "$SBF_BUILDER")
test -x "$builder"

output_dir=${1:-"$repo/target/ab-evidence"}
mkdir -p "$output_dir"
output_dir=$(cd "$output_dir" && pwd)

temporary_root=$(mktemp -d)
old_tree="$temporary_root/old"
current_tree="$temporary_root/current"

cleanup() {
    git -C "$repo" worktree remove --force "$old_tree" 2>/dev/null || true
    git -C "$repo" worktree prune
    rm -rf "$temporary_root"
}
trap cleanup EXIT

current_revision=$(git -C "$repo" rev-parse HEAD)
tracked_diff_hash=$(git -C "$repo" diff --binary HEAD -- . | git hash-object --stdin)
untracked_hash=$(git -C "$repo" ls-files --others --exclude-standard -z \
    | while IFS= read -r -d '' path; do
        printf '%s\0' "$path"
        git -C "$repo" hash-object "$path"
    done \
    | git hash-object --stdin)
current_state_hash=$(printf '%s\n%s\n' "$tracked_diff_hash" "$untracked_hash" | git hash-object --stdin)

git -C "$repo" worktree add --detach "$old_tree" "$old_revision"
mkdir -p "$old_tree/bench"
cp -R "$repo/bench/ab" "$old_tree/bench/ab"
cp "$old_tree/bench/ab/program/Cargo.old.toml" "$old_tree/bench/ab/program/Cargo.toml"

mkdir -p "$current_tree"
rsync -a \
    --exclude .git/ \
    --exclude target/ \
    --exclude bench/target/ \
    --exclude bench/ab/target/ \
    "$repo/" "$current_tree/"

old_harness_hash=$(shasum -a 256 "$old_tree/bench/ab/program/src/lib.rs" | awk '{print $1}')
current_harness_hash=$(shasum -a 256 "$current_tree/bench/ab/program/src/lib.rs" | awk '{print $1}')
test "$old_harness_hash" = "$current_harness_hash"
old_workloads_hash=$(shasum -a 256 "$old_tree/bench/ab/workloads.tsv" | awk '{print $1}')
current_workloads_hash=$(shasum -a 256 "$current_tree/bench/ab/workloads.tsv" | awk '{print $1}')
test "$old_workloads_hash" = "$current_workloads_hash"

{
    printf 'old_revision=%s\n' "$old_revision"
    printf 'current_revision=%s\n' "$current_revision"
    printf 'tracked_diff_hash=%s\n' "$tracked_diff_hash"
    printf 'untracked_hash=%s\n' "$untracked_hash"
    printf 'current_state_hash=%s\n' "$current_state_hash"
    printf 'shared_harness_sha256=%s\n' "$current_harness_hash"
    printf 'workloads_sha256=%s\n' "$current_workloads_hash"
    rustc +1.89.0 --version
    cargo +1.89.0 --version
    printf 'cargo_build_sbf_path=%s\n' "$builder"
    printf 'cargo_build_sbf_sha256=%s\n' "$(shasum -a 256 "$builder" | awk '{print $1}')"
    "$builder" --version
} > "$output_dir/provenance.txt"

(
    cd "$old_tree/bench/ab/program"
    "$builder" --no-default-features --features old
) 2>&1 | tee "$output_dir/old-build.txt"

(
    cd "$current_tree/bench/ab/program"
    "$builder" --no-default-features --features current
) 2>&1 | tee "$output_dir/current-build.txt"

old_so="$old_tree/bench/ab/target/deploy/svm_math_ab_program.so"
current_so="$current_tree/bench/ab/target/deploy/svm_math_ab_program.so"
test -s "$old_so"
test -s "$current_so"
cp "$old_so" "$output_dir/old.so"
cp "$current_so" "$output_dir/current.so"

cargo +1.89.0 run --release \
    --manifest-path "$repo/bench/ab/Cargo.toml" \
    --package svm-math-ab-runner \
    --bin svm-math-ab \
    -- \
    "$output_dir/old.so" \
    "$output_dir/current.so" \
    "$current_harness_hash" \
    "$old_revision" \
    "$current_revision" \
    "$current_state_hash" \
    | tee "$output_dir/results.md"

cargo +1.89.0 run --release \
    --manifest-path "$repo/bench/ab/Cargo.toml" \
    --package svm-math-ab-runner \
    --bin attribution \
    -- \
    "$output_dir/current.so" \
    "$current_harness_hash" \
    "$current_state_hash" \
    | tee "$output_dir/attribution.md"

printf 'A/B evidence written to %s\n' "$output_dir"
