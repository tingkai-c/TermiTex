# TermiTex versus TFormula: live-wrapper comparison

Measured October 7, 2026 on macOS 26.6 / arm64. This is a headless PTY experiment, not a Ghostty frame benchmark.

## Results

| Metric: median across five trials | TermiTex 0.1.0 | TFormula 0.3.1 |
| :--- | ---: | ---: |
| Process launch to first image placement | 18.57 ms | 361.11 ms |
| New-equation source to placement, excluding first | 5.19 ms | 203.85 ms |
| Repeated-equation source to placement | 0.49 ms | 196.75 ms |
| Peak sampled process-tree RSS | 20.88 MiB | 184.22 MiB |
| Validated PNG placements in every trial | 24 / 24 | 24 / 24 |

Launch ranges: 12.16–22.82 ms and 354.12–581.64 ms respectively. Trial-peak RSS ranges: 20.84–20.95 MiB and 174.70–184.41 MiB. See [raw trials](results.jsonl).

The new/repeated figures are the median of each trial's median. Memory is the median of each trial's sampled maximum. Do not combine these numbers with the older isolated-renderer CPU/peak-RSS benchmark: they have different process scope and measurement methods.

## Versions and setup

- TermiTex: release binary, RaTeX backend, `--layout compatibility`; rendering implementation from `9c31b71`, plus packaging-only version/help handling included in v0.1.0. Tested binary SHA-256: `47993cb653ddce6dfd13b9c7cf38b9394f09436ce3c6f33e964be409f64ffa77`. Built with Rust 1.95.0.
- TFormula: unmodified npm release 0.3.1, Node 26.10.0. The adjacent npm lockfile records resolved dependencies and package integrity. Defaults are retained, including scan/stability scheduling and the local Ghostty image-file transport.
- Harness: [Rust source](../../examples/compare_wrappers.rs), built in release mode with Rust 1.99.0. No Python required.
- Synthetic terminal: 100 columns × 30 rows, 16 × 34 px cells; Ghostty identity and responses to device, cell-size, window-size, color and Kitty graphics probes.
- Each wrapper runs the same fixture executable. It emits 12 display formulas, clears/redraws between cases, then repeats them in the same order. The corpus includes algebra, an integral, a sum, a quadratic formula, conditional probability, Maxwell notation, a square root and a matrix.
- Each next frame is sent 80 ms after a placement is received. This dwell is outside the recorded per-equation latency. It allows pending acknowledgments and memory sampling; it is not a scrolling simulation.
- Both tools use fresh application caches per trial. Existing user config is isolated. OS caches are not flushed. Order alternates between tools across the five trials.

## Measurement and validation

Launch latency starts before spawning the wrapper and ends when the first validated image placement arrives. Per-equation latency starts when that equation's `CASE` marker reaches the outer PTY. It therefore excludes any delay before the wrapper forwards that marker. Repeated formulas reuse each tool's normal caches; image upload counts need not be identical.

The harness decodes every PNG, checks dimensions and visible light pixels, and requires a validated image before counting a placement. It saves images and the complete ANSI transcript. Representative integral output from both tools was visually inspected. These checks establish successful nonblank output, not pixel-equivalent fonts or complete mathematical correctness for arbitrary input.

TFormula's local temporary-file uploads are read and acknowledged, preserving its preferred macOS/Ghostty path; TermiTex's direct uploads are assembled and decoded. Real GPU work is not simulated. Graphics acknowledgments are emulated; q=2 requests receive none. Source-code and timings should be considered together when reproducing this test.

Memory sampling runs `ps` for PID, parent PID and RSS, sums each live wrapper tree, then sleeps 20 ms between samples. Each sample also costs time, so the actual interval exceeds 20 ms. The identical fixture child is included; the controller, its `ps` subprocess, and a real terminal emulator are excluded. RSS can double-count shared pages and miss brief peaks. This is not kernel peak-RSS accounting or a CPU-time measurement.

## Limits

- These figures include scheduling policies, startup and IPC. They do not mean RaTeX typesetting itself is hundreds of times faster.
- First-ever launches can be much slower: a successful pre-measurement validation run took about 21.9 seconds to reach TFormula's first placement after a Node upgrade. Its cause was not isolated. Published trials followed that validation run; they are fresh application-cache measurements, not pristine-machine startup guarantees.
- No live terminal frame rate, input latency, mouse behavior, native scrollback, wrapped-inline corpus, cross-session warm disk cache, or large-document workload is measured.
- TXM 0.1.6 is an expression renderer, not an equivalent live PTY wrapper. It needs a separately specified renderer benchmark or integration layer; we do not present an N/A as a win.
- Keep failures and skipped cases in any expanded comparison. A faster result with missing equations is not a performance improvement.

## Reproduce

From the repository root, with Rust and Node.js 20+ installed:

```sh
cargo build --release --locked
cargo build --release --locked --example compare_wrappers
npm ci --prefix benchmarks/competitors
./target/release/examples/compare_wrappers \
  "$PWD/target/release/termitex" \
  "$(command -v node)" \
  "$PWD/benchmarks/competitors/node_modules/tformula/dist/cli.js" \
  "$PWD/benchmarks/competitors/artifacts/run-1" 5 \
  > benchmarks/competitors/artifacts-results.jsonl
```

Use a **new artifact directory per run** so persistent caches are empty. Native dependency setup scripts must be permitted for TFormula's node-pty dependency. The harness needs PTY creation and read-only process inspection; some sandboxes require explicit permission. The recorded defaults came from npm 0.3.1, not a patched local installation.
