SBF_BUILDER ?= cargo build-sbf
SBF_BUILDER_BIN := $(if $(filter cargo,$(word 1,$(SBF_BUILDER))),cargo-build-sbf,$(word 1,$(SBF_BUILDER)))
BENCH_CARGO ?= cargo +1.89.0
CODE_SIZE_FAMILIES := compound defi_amm defi_fee defi_lending defi_oracle defi_schedule defi_staking exp2 log2 pow powi sqrt wide
CODE_SIZE_FAMILY_MANIFEST := verification/sbf-code-size-families.txt
CODE_SIZE_BASELINE_MANIFEST := verification/sbf-code-size-baselines.txt

.PHONY: api check features oracle verify lean kani-list kani exhaustive benchmark code-size

api:
	@test "$$(cargo public-api --version)" = "cargo-public-api 0.52.0"
	@tmp_file=$$(mktemp); trap 'rm -f "$$tmp_file"' EXIT; \
		cargo public-api -sss --color never > "$$tmp_file"; \
		diff -u verification/public-api.txt "$$tmp_file"

check:
	cargo fmt --all -- --check
	cargo clippy --all-targets -- -D warnings
	cargo test
	cargo test --release
	cargo test --doc
	RUSTDOCFLAGS="-D warnings" cargo doc --no-deps
	cargo check --example core_consumer
	cargo check --example defi_consumer
	$(MAKE) api
	@tmp_file=$$(mktemp); trap 'rm -f "$$tmp_file"' EXIT; \
		cargo package --allow-dirty --list | LC_ALL=C sort > "$$tmp_file"; \
		diff -u verification/package-files.txt "$$tmp_file"
	cargo package --allow-dirty

features:
	cargo check --no-default-features
	cargo metadata --no-deps --format-version 1 | \
		jq -e '.packages | length == 1 and .[0].features == {} and .[0].dependencies == []' >/dev/null

oracle:
	cargo run --release --manifest-path verification/oracle/Cargo.toml

verify: check features oracle

lean:
	cd verification/lean && lean Sqrt.lean

kani-list:
	@actual=$$(mktemp); expected=$$(mktemp); \
		trap 'rm -f "$$actual" "$$expected"' EXIT; \
		awk '/^#\[kani::proof\]$$/ { proof = 1; next } proof && /^fn [a-z0-9_]+\(/ { \
			name = $$2; sub(/\(.*/, "", name); print name; proof = 0 \
		}' src/proofs.rs | LC_ALL=C sort > "$$actual"; \
		test "$$(rg -c '^[a-z0-9_]+ unwind=[0-9]+ solver=[a-zA-Z0-9_-]+$$' verification/kani-harnesses.txt)" = 13; \
		cut -d' ' -f1 verification/kani-harnesses.txt | LC_ALL=C sort > "$$expected"; \
		test "$$(uniq "$$expected" | wc -l | tr -d ' ')" = 13; \
		diff -u "$$expected" "$$actual"

kani: kani-list
	@set -eu; \
		out=target/plan-002-evidence/kani; mkdir -p "$$out"; \
		: > "$$out/summary.txt"; \
		while read -r name unwind_field solver_field; do \
			unwind=$${unwind_field#unwind=}; solver=$${solver_field#solver=}; \
			command="cargo kani --exact --harness proofs::$$name --unwind $$unwind --solver $$solver"; \
			printf '%s\n' "$$command" > "$$out/$$name.command.txt"; \
			start=$$(date +%s); \
			if cargo kani --exact --harness "proofs::$$name" --unwind "$$unwind" --solver "$$solver" \
				> "$$out/$$name.log" 2>&1; then result=pass; else result=fail; fi; \
			end=$$(date +%s); elapsed=$$((end - start)); \
			printf '%s unwind=%s solver=%s result=%s elapsed_seconds=%s\n' \
				"$$name" "$$unwind" "$$solver" "$$result" "$$elapsed" | tee -a "$$out/summary.txt"; \
			test "$$result" = pass; \
		done < verification/kani-harnesses.txt

exhaustive:
	@actual=$$(mktemp); expected=$$(mktemp); \
		trap 'rm -f "$$actual" "$$expected"' EXIT; \
		cargo test --release --lib -- --ignored --list \
			| sed -n 's/: test$$//p' | LC_ALL=C sort > "$$actual"; \
		LC_ALL=C sort verification/exhaustive-tests.txt > "$$expected"; \
		test "$$(wc -l < "$$expected" | tr -d ' ')" = 3; \
		diff -u "$$expected" "$$actual"; \
		while IFS= read -r test_name; do \
			cargo test --release --lib "$$test_name" -- --ignored --exact; \
		done < "$$expected"

benchmark:
	@printf 'git revision: '
	@git rev-parse --verify HEAD
	@if git diff --quiet HEAD -- && test -z "$$(git ls-files --others --exclude-standard)"; then \
		printf 'git tree state: clean\n'; \
	else \
		printf 'git tree state: dirty (tracked-diff=%s untracked-list=%s)\n' \
			"$$(git diff --binary HEAD | git hash-object --stdin)" \
			"$$(git ls-files --others --exclude-standard | git hash-object --stdin)"; \
	fi
	@rustc --version
	@cargo --version
	@$(SBF_BUILDER) --version
	cd bench/program && $(SBF_BUILDER)
	cd bench && SVM_MATH_REPETITIONS="$${SVM_MATH_REPETITIONS:-3}" $(BENCH_CARGO) run --release --bin cu
	@if [ -n "$$SVM_MATH_CHECK_CODE_SIZE" ]; then \
		$(MAKE) --no-print-directory code-size; \
	fi

code-size:
	@set -eu; \
		test "$${SVM_MATH_CHECK_CODE_SIZE:-}" = 1; \
		phase="$${SVM_MATH_EVIDENCE_PHASE:-}"; \
		test "$$phase" = baseline || test "$$phase" = final; \
		expected='$(CODE_SIZE_FAMILIES)'; \
		families=$$(awk 'NF && $$1 !~ /^#/ { \
			if (NF != 1 || $$1 !~ /^[a-z0-9_]+$$/ || seen[$$1]++) exit 1; \
			print $$1; n++ \
		} END { exit n != 13 }' $(CODE_SIZE_FAMILY_MANIFEST)); \
		test "$$families" = "$$(printf '%s\n' $$expected)"; \
		baseline_families=$$(awk 'NF && $$1 !~ /^#/ { \
			if (NF != 4 || $$1 !~ /^[a-z0-9_]+$$/ || $$2 !~ /^[0-9]+$$/ || $$3 !~ /^[0-9]+$$/ || $$4 !~ /^[0-9]+$$/ || seen[$$1]++) exit 1; \
			print $$1; n++ \
		} END { exit n != 13 }' $(CODE_SIZE_BASELINE_MANIFEST)); \
		test "$$baseline_families" = "$$families"; \
		builder_bin="$$(command -v $(SBF_BUILDER_BIN) || true)"; \
		test -n "$$builder_bin"; \
		builder_version="$$( $(SBF_BUILDER) --version )"; \
		tools_version="$$(printf '%s\n' "$$builder_version" | awk '/^platform-tools v[0-9.]+$$/ { print $$2 }')"; \
		test -n "$$tools_version"; \
		sbf_sdk="$${SBF_SDK_PATH:-}"; \
		if test -z "$$sbf_sdk" && test -d "$$(dirname "$$builder_bin")/platform-tools-sdk/sbf"; then \
			sbf_sdk="$$(cd "$$(dirname "$$builder_bin")/platform-tools-sdk/sbf" && pwd)"; \
		fi; \
		cache_root="$${SOLANA_PLATFORM_TOOLS_DIR:-$${XDG_CACHE_HOME:-$$HOME/.cache}/solana/$$tools_version/platform-tools}"; \
		if test -n "$${LLVM_SIZE:-}"; then \
			tool="$$LLVM_SIZE"; \
			platform_tools_root="$$(cd "$$(dirname "$$tool")/../../.." && pwd)"; \
		elif test -d "$$cache_root"; then \
			platform_tools_root="$$cache_root"; \
			tool=$$(find "$$platform_tools_root/rust/lib/rustlib" -type f -path '*/bin/llvm-size' -perm -111 -print 2>/dev/null); \
		elif test -d "$$sbf_sdk/dependencies/platform-tools"; then \
			platform_tools_root="$$sbf_sdk/dependencies/platform-tools"; \
			tool=$$(find "$$platform_tools_root/rust/lib/rustlib" -type f -path '*/bin/llvm-size' -perm -111 -print 2>/dev/null); \
		else \
			false; \
		fi; \
		set -- $$tool; test $$# = 1; tool="$$1"; test -x "$$tool"; \
		out="target/plan-002-evidence/$$phase/code-size"; \
		mkdir -p "$$out"; \
		cp $(CODE_SIZE_BASELINE_MANIFEST) "$$out/reviewed-baselines.txt"; \
		{ \
			printf 'phase=%s\n' "$$phase"; \
			printf 'git_revision=%s\n' "$$(git rev-parse --verify HEAD)"; \
			printf 'git_diff_hash=%s\n' "$$(git diff --binary HEAD | git hash-object --stdin)"; \
			printf 'git_untracked_hash=%s\n' "$$(git ls-files --others --exclude-standard | git hash-object --stdin)"; \
			printf 'sbf_builder=%s\n' '$(SBF_BUILDER)'; \
			printf 'sbf_builder_version:\n%s\n' "$$builder_version"; \
			printf 'sbf_sdk=%s\n' "$$sbf_sdk"; \
			printf 'platform_tools_root=%s\n' "$$platform_tools_root"; \
			printf 'llvm_size=%s\n' "$$tool"; \
			printf 'llvm_size_version='; "$$tool" --version | head -n 1; \
		} > "$$out/provenance.txt"; \
		: > "$$out/commands.txt"; \
		: > "$$out/measurements.txt"; \
		: > "$$out/comparison.txt"; \
		for family in $(CODE_SIZE_FAMILIES); do \
			so=bench/target/deploy/svm_math_bench_program.so; \
			rm -f "$$so"; test ! -e "$$so"; \
			printf '%s\n' "cd bench/program && $(SBF_BUILDER) --features size-$$family" >> "$$out/commands.txt"; \
			(cd bench/program && $(SBF_BUILDER) --features size-$$family) > "$$out/$$family.build.txt" 2>&1; \
			test -s "$$so"; \
			printf '%s\n' "$$tool -A $$so" >> "$$out/commands.txt"; \
			"$$tool" -A "$$so" > "$$out/$$family.llvm-size-A.txt"; \
			values=$$(awk '\
				$$1 == ".text" { if (++text_rows != 1 || $$2 !~ /^[0-9]+$$/) exit 1; text = $$2 } \
				$$1 == ".rodata" { if (++rodata_rows != 1 || $$2 !~ /^[0-9]+$$/) exit 1; rodata = $$2 } \
				$$1 == "Total" { if (++total_rows != 1 || $$2 !~ /^[0-9]+$$/) exit 1; total = $$2 } \
				END { if (text_rows != 1 || rodata_rows != 1 || total_rows != 1) exit 1; print text, rodata, total }' \
				"$$out/$$family.llvm-size-A.txt"); \
			set -- $$values; test $$# = 3; \
			printf '%s %s %s %s\n' "$$family" "$$1" "$$2" "$$3" >> "$$out/measurements.txt"; \
			if test "$$phase" = final; then \
				baseline_values=$$(awk -v family="$$family" '$$1 == family { print $$2, $$3, $$4 }' $(CODE_SIZE_BASELINE_MANIFEST)); \
				set -- $$baseline_values; test $$# = 3; baseline_text="$$1"; baseline_rodata="$$2"; baseline_total="$$3"; \
				set -- $$values; current_text="$$1"; current_rodata="$$2"; current_total="$$3"; \
				printf '%s text=%s reference_text=%s delta_text=%+d rodata=%s reference_rodata=%s delta_rodata=%+d total=%s reference_total=%s delta_total=%+d\n' \
					"$$family" "$$current_text" "$$baseline_text" "$$((current_text - baseline_text))" \
					"$$current_rodata" "$$baseline_rodata" "$$((current_rodata - baseline_rodata))" \
					"$$current_total" "$$baseline_total" "$$((current_total - baseline_total))" \
					>> "$$out/comparison.txt"; \
			fi; \
		done
