# Reproducible SBF A/B

`run.sh` compares the frozen `f589fc2` implementation with an immutable copy
of the current worktree. It builds both with the same SBF builder and Rust
toolchain, then loads both artifacts into one LiteSVM runner.

The shared program owns every raw operand, `black_box` placement, matched
control, 100-iteration loop, and output checksum. Revision adapters contain
only API construction, the operation call, and primitive output extraction.
The runner performs three measurements, requires exact raw equality for exact
and DeFi rows, and reports directed workflows without relabeling them as
kernel-only comparisons.

Run from anywhere inside the repository:

```sh
SBF_BUILDER=/path/to/cargo-build-sbf bench/ab/run.sh
```

Pass an explicit evidence directory as the first argument to retain artifacts
elsewhere. The default is `target/ab-evidence`. `SBF_BUILDER` is required; the
script resolves it once, uses that exact executable for both artifacts, and
records its path, SHA-256, and version in provenance.
