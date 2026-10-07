# TermiTex

Asynchronous terminal math rendering for stock Codex CLI in Ghostty on macOS. No custom Codex build.

**RaTeX is the default.** It runs natively in Rust with embedded math fonts, without Node.js. MathJax remains available as an explicitly selected alternative. TFormula is not a dependency.

**Status: early prototype.** The native backend passed a representative rendering and failure-recovery gate, and both backends passed headless PTY scrolling/resize checks. This does not establish complete LaTeX compatibility or replace testing in live Ghostty. The earlier MathJax backend received positive user feedback on scrolling.

## Build and run

Requires a recent Rust toolchain (tested with 1.95), Ghostty, and Codex CLI on PATH.

```sh
cargo build --release --locked
./target/release/termitex
```

With no command, TermiTex launches `codex -c tui.rendering.math=false`. To pass Codex options:

```sh
./target/release/termitex -- codex -c tui.rendering.math=false --model MODEL
```

The native binary embeds its math fonts and does not need the checkout or npm at runtime. Keep the bundled [font license and notices](licenses/) with redistributed binaries. Existing Codex installations are not modified.

## Choose a renderer

Create `~/.config/termitex/config.toml`, or `$XDG_CONFIG_HOME/termitex/config.toml` when that variable is set:

```toml
renderer = "ratex" # or "mathjax"
```

Selection precedence: **`--renderer` > `TERMITEX_RENDERER` > config file > RaTeX default**. Invalid renderer names and unknown config keys produce an error. See [config.example.toml](config.example.toml).

```sh
termitex --renderer ratex
TERMITEX_RENDERER=mathjax termitex
termitex --renderer mathjax -- codex -c tui.rendering.math=false
```

Options after `--` (or after the child command begins) belong to the child command.

| Backend | Use it for | Requirements |
|---|---|---|
| RaTeX (default) | Standard math, matrices, aligned equations, chemistry; lower measured renderer overhead | Native binary only; system fonts for CJK fallback |
| MathJax | MathJax-specific syntax/extensions, e.g. physics `\dv`, or a preferred appearance | Node.js 20+, npm dependencies, and the worker files in the build checkout |

For MathJax, install its direct dependencies in the checkout:

```sh
npm ci --ignore-scripts
./target/release/termitex --renderer mathjax
```

Only the selected worker is launched. There is **no automatic fallback**: unsupported or oversized formulas remain readable source text. Backend identity is included in image-cache keys. RaTeX uses KaTeX-compatible syntax and fonts; appearance and extension coverage differ from MathJax. Its default status reflects the tested standard-math cases, not full MathJax parity.

## Design

Rust owns the PTY, screen model, formula detection, layout, cache, and Kitty image placements. Both backends use the same bounded request/response interface. RaTeX runs in an isolated native child process; MathJax uses a single prewarmed Node worker with direct MathJax/resvg calls. A renderer stall does not block the PTY loop; stalled workers are disabled after the existing timeout.

- Explicit delimiters: `\(…\)`, `\[…\]`, `$…$`, and `$$…$$`. Dollar inline math currently requires an operator or command.
- Inline images use rendered width rather than LaTeX source width. Following prose moves left as native terminal text, preserving styles and Unicode.
- Visible wrapped inline formulas are joined across at most nine source rows. The equation is placed intact on the row with the most room; other source fragments disappear. Partially visible formulas remain as text.
- One outstanding render request bounds work. Delayed results are matched to the current viewport, and cached images move without typesetting again.
- The shared image cache targets 128 images / 64 MiB of encoded PNGs, retaining currently visible images. MathJax additionally has a 16 MiB worker cache and a bounded 128 MiB checksummed disk cache. RaTeX uses its font/glyph caches and the shared session image cache; it does not currently persist final images between sessions.
- Fenced code and the detected Codex input area are excluded heuristically.

## Limits and settings

Targets Codex redraws in Ghostty, not arbitrary terminal applications or native terminal scrollback. Inline compaction does not reflow whole paragraphs. Runs of two or more source spaces are treated as alignment padding, preserving table columns while compacting math within each cell. Composer detection depends on visible prompt markers. Indexed colors fall back to configured defaults. Large equations may be scaled or left as source; TermiTex does not allocate extra rows. Terminal control coverage and cursor edge cases need broader testing.

Unlike a source-preserving overlay, inline compaction rewrites displayed terminal cells and moves nearby prose. Mouse coordinates are forwarded unchanged, so clicking shifted content in a mouse-driven application may target the wrong original cell. Selection/copy sees projected text, not necessarily original LaTeX. Applications using terminal state our screen model does not fully represent (such as hyperlinks or advanced text attributes) may lose that state on repainted rows. Misdetected editable text and unsupported redraw behavior can disrupt interaction. Use with arbitrary editors or TUIs is not yet validated; the underlying application data is not directly rewritten.

`TERMITEX_FG` / `TERMITEX_BG` set hex colors (defaults `#ffffff` / `#282c34`). `TERMITEX_STATS=/tmp/termitex-stats.json` writes content-free counters at exit. MathJax-only options: `TERMITEX_PREWARM=0` disables prewarming; `TERMITEX_CACHE_DIR` overrides its cache directory (`~/Library/Caches/termitex/v1` by default on macOS). Legacy `TFORMULA_*` settings are not read.

## Validation

```sh
cargo test --locked
cargo fmt --check
cargo build --release --locked
```

Optional MathJax checks (after npm installation):

```sh
cargo test --locked --test workers mathjax -- --ignored
node tests/backend_pixels.mjs
node tests/cache.mjs
cargo test --locked --test pty mathjax -- --ignored
```

The Rust integration tests launch the Cargo-built binary; Python and Pillow are not needed for tests. Set `TERMITEX_TEST_ARTIFACTS=/tmp/termitex-qa` to save rendered PNGs and their source requests for visual inspection. Historical benchmark scripts still use Python.

The native gate covers 31 renders: Maxwell equations, fractions, integrals, probability, matrices, aligned/cases environments, chemistry, accents, custom macros, Chinese text, and changed cell metrics. It verifies PNG dimensions, visible ink, five invalid/unsupported inputs, and recovery, with Node absent from PATH. Representative images were visually inspected. PTY tests cover inline/wrapped placement across scrolling, redraws, and resizing. Rust tests cover configuration precedence, syntax detection, native color validation, stale responses, and native-text projection. These are not live Ghostty frame benchmarks.

See [RaTeX comparison](benchmarks/ratex/README.md) for the isolated benchmark and its limitations, and [earlier backend measurements](benchmarks/README.md). Measurements exclude Codex and Ghostty and are not total-application guarantees.

## Attribution

[RaTeX](https://github.com/erweixin/RaTeX) is pinned to commit `776c1d37bafa3bf445a0ab9377c55fe77f7a0133`. Its MIT notice and the embedded KaTeX fonts' OFL license/notices are in [licenses](licenses/).

Originally inspired by [TFormula](https://github.com/mikewang817/TFormula), by Mike Wang. Small MathJax geometry, SVG-dimension, and TeX-compatibility helpers were adapted under MIT; the notice remains in `worker/TFORMULA-LICENSE`. No TFormula package, process, configuration, or cache is used. MathJax/resvg retain their own licenses. TermiTex is MIT licensed.
