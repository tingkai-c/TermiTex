<p align="center">
  <img src="assets/termitex-icon.png" width="128" height="128" alt="TermiTex terminal and summation icon">
</p>
<h1 align="center">TermiTex</h1>
<p align="center"><strong>Beautiful math, right in your terminal.</strong></p>
<p align="center">A Rust math renderer for live CLI output. Built for stock Codex in Ghostty.</p>
<p align="center">
  <a href="LICENSE"><img src="https://img.shields.io/badge/license-MIT-green" alt="MIT license"></a>
  <img src="https://img.shields.io/badge/status-prototype-orange" alt="Early prototype">
  <img src="https://img.shields.io/badge/default_renderer-native_Rust-blue" alt="Native Rust renderer by default">
</p>
<p align="center">
  <a href="#quick-start">Quick start</a> ·
  <a href="#performance">Performance</a> ·
  <a href="#comparison">Comparison</a> ·
  <a href="#configuration">Configuration</a> ·
  <a href="#contributing">Contributing</a>
</p>

Turn LaTeX in terminal output into typeset equations without a custom Codex build. TermiTex tracks the visible screen, renders math in a separate worker, and repositions cached images as the application redraws.

- **Native by default.** RaTeX renders with embedded math fonts; no Node.js or TFormula runtime required.
- **Rendering stays off the input loop.** Cached equations move without being typeset again.
- **Two layouts.** Compact math with native prose for Codex, or centered overlays that preserve source cells.
- **Optional MathJax.** Select it for its appearance or extensions such as physics `\dv`.

**Early prototype:** developed on macOS with Ghostty. Other terminal applications and terminals need broader testing. See [known limits](#known-limits).

## Quick start

On macOS, with [Homebrew](https://brew.sh), Ghostty, and Codex CLI installed:

```sh
brew install tingkai-c/tap/termitex
termitex
```

Homebrew builds the native renderer from a pinned release and installs Rust as a build dependency automatically. The initial install may take a few minutes; Node and npm are not required. This is our [project tap](https://github.com/tingkai-c/homebrew-tap), not a Homebrew/core formula.

To update later:

```sh
brew update
brew upgrade termitex
```

<details>
<summary>Build from source instead</summary>

Requires Git and a recent Rust toolchain (tested with 1.95).

```sh
git clone https://github.com/tingkai-c/TermiTex.git
cd TermiTex
cargo build --release --locked
./target/release/termitex
```

</details>

The default command launches `codex -c tui.rendering.math=false`. To pass your own Codex options:

```sh
termitex -- codex -c tui.rendering.math=false --model MODEL
```

For another interactive program, opt into compatibility mode:

```sh
termitex --layout compatibility -- your-program
```

The native binary can run outside the checkout. Keep the [font licenses and notices](licenses/) with redistributed binaries. Existing Codex installations are not modified.

## Performance

**TermiTex vs TFormula, running the same live-terminal workload.** Both wrappers rendered all 24 cases in each trial: 12 different display equations, then the same 12 again.

| Metric · median across 5 trials | TermiTex 0.1.0 · native, compatibility mode | TFormula 0.3.1 · default settings |
| :--- | ---: | ---: |
| Launch → first image placement | **18.57 ms** | 361.11 ms |
| New equation → placement, after first render | **5.19 ms** | 203.85 ms |
| Repeated equation → placement | **0.49 ms** | 196.75 ms |
| Peak sampled process-tree RSS | **20.88 MiB** | 184.22 MiB |
| Cases producing validated PNG placements per trial | 24 / 24 | 24 / 24 |

Measured October 7, 2026, on macOS 26.6 / arm64 with Node 26.10.0. Five alternating trials, fresh application caches, a 100 × 30 synthetic terminal, and 16 × 34 px cells. Memory includes the wrapper, renderer descendants, and the identical fixture child; it excludes the benchmark controller and terminal emulator.

**Scope:** placement means the image command reached our headless terminal harness—not that Ghostty displayed a frame. New/repeated latency starts when source text reaches that harness. TFormula's default scan/stability scheduling is included; this is not a pure typesetting-speed comparison. Process-tree RSS is sampled, can miss peaks, and can double-count shared pages. This is not a CPU-time or broad compatibility benchmark. TXM renders supplied expressions rather than wrapping this workload, so it is not assigned an incomparable timing.

[Reproduce the comparison](benchmarks/competitors/README.md) · [Raw trials](benchmarks/competitors/results.jsonl) · [Earlier internal renderer benchmark](benchmarks/ratex/README.md)

## Comparison

Different tools cover different workflows. Competitor capabilities below come from their upstream documentation, reviewed October 7, 2026; they are not results of our compatibility suite.

| Tool | Main workflow | Rendering and runtime | Performance evidence available here |
| :--- | :--- | :--- | :--- |
| **TermiTex / RaTeX** | Live PTY wrapper; compact or source-preserving layout | Native Rust worker; embedded math fonts | Live-wrapper placement and sampled memory measurements above |
| **TermiTex / MathJax** | Same wrapper and layouts; alternative math syntax coverage | Node.js with MathJax/resvg | [Separate internal worker benchmark](benchmarks/ratex/README.md); not included above |
| [**TFormula**](https://github.com/mikewang817/TFormula#tformula) | Live agent wrapper plus a document reader; source-preserving overlays and wrapped-formula slices | MathJax; reusable image placements and persistent caching | Live-wrapper placement and sampled memory measurements above; no live frame-time measurement |
| [**TXM**](https://github.com/thatmagicalcat/txm#txm) | Render supplied LaTeX expressions; library/editor integration | Rust CLI and library; Python bindings available | Not benchmarked here; expression rendering is a different workload from live PTY tracking |

**How to read this:** TFormula's persistent cache can avoid repeat rendering across sessions; our native backend currently caches final images only within a session. TXM's expression renderer needs an integration layer to be evaluated on the same live-terminal workload. Runtime language alone cannot establish which complete application is faster or more compatible.

These results establish lower placement latency and sampled memory for this workload and configuration. They do **not** establish broader compatibility or faster live scrolling. See [comparison scope](docs/comparison.md).

## Configuration

Create `~/.config/termitex/config.toml` (or `$XDG_CONFIG_HOME/termitex/config.toml`):

```toml
renderer = "ratex" # "ratex" or "mathjax"
layout = "compact" # "compact" or "compatibility"
```

| Setting | Default | Override |
| :--- | :--- | :--- |
| Renderer | `ratex` | `--renderer mathjax` or `TERMITEX_RENDERER=mathjax` |
| Layout | `compact` | `--layout compatibility` or `TERMITEX_LAYOUT=compatibility` |
| Foreground / background | `#ffffff` / `#282c34` | `TERMITEX_FG` / `TERMITEX_BG` |
| Statistics file | Disabled | `TERMITEX_STATS=/tmp/termitex-stats.json` |

Precedence: **CLI > environment > config > default**. Options after `--`, or after the child command starts, belong to that program. See [config.example.toml](config.example.toml).

### Choose a layout

| Behavior | Compact (default) | Compatibility (opt-in) |
| :--- | :--- | :--- |
| Inline math | Fits rendered width; surrounding prose moves left | Centered horizontally in the original source span; text baseline retained |
| Display math | Centered in its detected block | Centered in its detected block |
| Underlying terminal text | Projected cells replace visible source | Original source cells remain intact |
| Wrapped inline formulas | Joined when fully visible, up to nine source rows | Left as source to avoid covering neighboring prose |
| Intended use | Codex in Ghostty | Trying other interactive applications; not universally validated |

### Use MathJax

The Homebrew package includes the native renderer only. For MathJax, use the source-build instructions above, keep the checkout, and install Node.js 20+. From that checkout:

```sh
npm ci --ignore-scripts
./target/release/termitex --renderer mathjax
```

Only the selected worker starts. There is no automatic fallback: unsupported or oversized formulas remain source text. RaTeX and MathJax differ in appearance and extension coverage. [Rendering details and cache settings](docs/reference.md).

## Known limits

- Requires Kitty graphics support; development and testing focus on Ghostty/macOS. Native terminal scrollback is not fully supported.
- Input-area and code detection are heuristic. Compatibility mode preserves coordinates but does not guarantee correctness in arbitrary editors or TUIs.
- Compact mode changes visible text positions. Mouse clicks are forwarded at original coordinates, and copying may omit replaced LaTeX. It does not reflow entire paragraphs.
- Large formulas may be scaled or left as source. TermiTex does not allocate extra rows. Indexed colors use configured defaults; advanced terminal attributes need broader coverage.
- The wrapper currently does not propagate the child program's exit code. Do not use its exit status to determine whether the child succeeded.

[Detailed behavior and limitations](docs/reference.md#limits-and-settings)

## Contributing

Bug reports are most useful with the terminal and OS versions, backend/layout, a minimal formula or redraw sequence, and expected versus actual output. [Open an issue](https://github.com/tingkai-c/TermiTex/issues).

```sh
cargo test --locked
cargo fmt --check
```

The Rust tests cover detection, layout, configuration, worker recovery, and headless PTY scrolling/resizing. Native image checks exercise 31 representative renders and five invalid/unsupported inputs; compatibility checks verify centering. No Python or Pillow is needed for tests. Historical benchmark scripts still use Python.

After installing npm dependencies, also run:

```sh
cargo test --locked --test workers mathjax -- --ignored
cargo test --locked --test pty mathjax -- --ignored
node tests/backend_pixels.mjs
node tests/cache.mjs
```

Set `TERMITEX_TEST_ARTIFACTS=/tmp/termitex-qa` to save PNGs and source requests for inspection. Headless checks are not live Ghostty frame benchmarks.

## License and acknowledgments

TermiTex is [MIT licensed](LICENSE). Native rendering uses [RaTeX](https://github.com/erweixin/RaTeX), pinned to `776c1d37bafa3bf445a0ab9377c55fe77f7a0133`; font notices are in [licenses/](licenses/).

Inspired by [TFormula](https://github.com/mikewang817/TFormula), by Mike Wang. Small MathJax geometry, SVG-dimension, and TeX-compatibility helpers were adapted under MIT; see [the retained notice](worker/TFORMULA-LICENSE). No TFormula package, process, configuration, or cache is used. MathJax/resvg retain their own licenses.
