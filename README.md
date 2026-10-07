# termitex

Experimental asynchronous terminal math rendering for stock Codex CLI in Ghostty on macOS. No custom Codex build.

**Status: early prototype.** Automated screen-model tests pass, and an initial user trial reported smooth scrolling in Ghostty. Rendering and layout remain under active development. This is not a finished replacement for TFormula.

## Build and run

Requires Rust (edition 2024), Node.js 20+, npm, Ghostty, and Codex CLI on PATH.

```sh
npm ci --ignore-scripts
cargo build --release
./target/release/termitex
```

With no arguments, launches `codex -c tui.rendering.math=false`. To pass Codex options:

```sh
./target/release/termitex -- codex -c tui.rendering.math=false --model MODEL
```

Keep the checkout in place: the binary currently locates the worker relative to its build directory. Existing Codex and TFormula installations are not modified.

## Design

Rust owns the PTY, screen model, formula detection, cache, and Kitty image placements. A single background Node worker uses **TFormula 0.3.1's MathJax adapter** for SVG and calls resvg directly for PNG output. It prewarms MathJax and native rasterization at startup. This is not yet an all-Rust typesetter.

- Only explicit math delimiters are detected: `\(…\)`, `\[…\]`, `$…$`, and `$$…$$`. Dollar inline math currently requires a math operator or command.
- Inline images use their rendered width instead of reserving the entire LaTeX source width. Following prose moves left as native terminal text, retaining its styles and Unicode characters. A separate screen model tracks projected text so partial redraws and scrolling can be reconciled.
- Inline canvas width is determined before rasterization: one PNG encoding pass, without Sharp cropping or a second raster-worker process. The backend caches final PNGs and SVGs with a 16 MiB memory budget plus the existing disk cache.
- One outstanding render request bounds work. Output processing never waits for a render result. Results are matched to the current screen before placement.
- Cached images are repositioned without rerunning MathJax. Synchronized output groups text and image placement updates.
- Cache eviction targets 128 images / 64 MiB of encoded PNG data; currently visible images are retained.
- Fenced code and the detected Codex input area are excluded heuristically.

Moving typesetting off the PTY path does not by itself guarantee smooth scrolling. Screen parsing, image upload, placement, and Ghostty's own drawing still have costs. This prototype repositions overlays; it does not have privileged access to Codex's scroll model.

## Current limitations

Targets Codex redraws in Ghostty, not arbitrary terminal applications or native terminal scrollback. Fully visible inline formulas spanning up to nine source rows are joined and placed intact on the source row with the most room; remaining source fragments are removed. Blank lines, prompt markers, headings, and code fences stop continuation detection. Partially visible formulas remain as source text. Inline compaction stays within the original row; it does not reflow paragraphs. Composer detection depends on visible prompt markers. Indexed terminal colors fall back to configured defaults. Large formulas may be scaled or left as text; the wrapper does not allocate extra text rows. Terminal control coverage and cursor edge cases need broader testing. Malformed or long synchronized frames are released after a bounded wait. Do not assume every terminal application is transparently supported.

Defaults are white text on `#282c34`. Override with `TERMITEX_FG` / `TERMITEX_BG`. `TERMITEX_PREWARM=0` disables eager initialization for benchmarking. `TERMITEX_TFORMULA` can point to an existing TFormula package directory. `TERMITEX_STATS=/tmp/termitex-stats.json` writes counters at exit, without conversation text.

## Checks

```sh
cargo test
python3 tests/worker_smoke.py
node tests/backend_pixels.mjs
python3 tests/pty_smoke.py
```

Tests cover delimiter detection, Unicode columns, composer/code exclusion, split synchronized redraws, a stalled worker, stale results after screen movement, inline compaction with native styles, wrapped inline math with both soft wraps and explicit row redraws, partial redraws, and avoiding redundant uploads/placements. The worker smoke test renders real inline and display equations. A headless PTY integration test checks actual image placement after a redraw. Nine representative equations also have decoded-pixel parity checks against the previous pipeline. These are not live Ghostty visual or scrolling benchmarks. See [backend measurements](benchmarks/README.md).

## Attribution

Inspired by and currently reuses [TFormula](https://github.com/mikewang817/TFormula), by Mike Wang, under the MIT license. Its license is retained in `worker/TFORMULA-LICENSE`. The dependency tree pins patched Sharp and KaTeX via npm overrides; Sharp is only used by the reference comparison test, not the active worker. Termitex is MIT licensed; dependencies retain their respective licenses.
