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

Requires Git, a recent Rust toolchain (tested with 1.95), Ghostty, and Codex CLI on `PATH`.

```sh
git clone https://github.com/tingkai-c/TermiTex.git
cd TermiTex
cargo build --release --locked
./target/release/termitex
```

The default command launches `codex -c tui.rendering.math=false`. To pass your own Codex options:

```sh
./target/release/termitex -- codex -c tui.rendering.math=false --model MODEL
```

For another interactive program, opt into compatibility mode:

```sh
./target/release/termitex --layout compatibility -- your-program
```

The native binary can run outside the checkout. Keep the [font licenses and notices](licenses/) with redistributed binaries. Existing Codex installations are not modified.

## Performance

**The native renderer used about 95% less CPU time and 87% less peak memory than our MathJax worker in an isolated benchmark.** These are renderer measurements, not total terminal resource savings or a head-to-head TFormula result.

| Metric | Native RaTeX benchmark driver | TermiTex MathJax worker | Difference |
| :--- | ---: | ---: | ---: |
| CPU time, complete 81-equation process | 16.38 ms | 319.08 ms | 94.9% lower |
| Peak process memory (RSS) | 14.84 MiB | 114.13 MiB | 87.0% lower |
| Warm request, new equation | 0.093 ms | 1.50 ms | About 16× faster |
| Process launch to first PNG | 4.41 ms | 121.19 ms | Median; variable cold starts |

Medians across five fresh-process trials per backend on local macOS arm64, measured October 7, 2026. Each trial rendered 81 distinct expressions at matched canvas sizes. Node v25.9.0; Rust 1.95.0; pinned RaTeX 0.1.14. MathJax used fresh disk caches. The native driver predates the integrated backend.

First-response outliers reached **584.8 ms for RaTeX and 371.8 ms for MathJax**. Measurements exclude the PTY frontend, Codex, Ghostty, and live display latency. Cached scrolling does not incur a fresh typesetting request, so these ratios do not describe scrolling speed.

[Methodology and reproduction](benchmarks/ratex/README.md) · [Raw measurements](benchmarks/ratex/results.jsonl) · [Earlier backend measurements](benchmarks/README.md)

## Comparison

Different tools cover different workflows. Competitor capabilities below come from their upstream documentation, reviewed October 7, 2026; they are not results of our compatibility suite.

| Tool | Main workflow | Rendering and runtime | Performance evidence available here |
| :--- | :--- | :--- | :--- |
| **TermiTex / RaTeX** | Live PTY wrapper; compact or source-preserving layout | Native Rust worker; embedded math fonts | Isolated native driver measurements above; production end-to-end comparison pending |
| **TermiTex / MathJax** | Same wrapper and layouts; alternative math syntax coverage | Node.js with MathJax/resvg | Worker measurements above; not a proxy for TFormula performance |
| [**TFormula**](https://github.com/mikewang817/TFormula#tformula) | Live agent wrapper plus a document reader; source-preserving overlays and wrapped-formula slices | MathJax; reusable image placements and persistent caching | No matched end-to-end CPU, memory, or frame-time measurement here |
| [**TXM**](https://github.com/thatmagicalcat/txm#txm) | Render supplied LaTeX expressions; library/editor integration | Rust CLI and library; Python bindings available | Not benchmarked here; expression rendering is a different workload from live PTY tracking |

**How to read this:** TFormula's persistent cache can avoid repeat rendering across sessions; our native backend currently caches final images only within a session. TXM's expression renderer needs an integration layer to be evaluated on the same live-terminal workload. Runtime language alone cannot establish which complete application is faster or more compatible.

We do **not** claim broader compatibility or faster scrolling than TFormula. A fair comparison must pin versions, use identical input and terminal geometry, separate cold/warm caches, count all child processes, and verify output correctness before comparing latency. See [comparison scope](docs/comparison.md).

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

Requires Node.js 20+ and the worker files in the build checkout:

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
