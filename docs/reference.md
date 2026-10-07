# Rendering reference

[Back to the README](../README.md)

## Design

Rust owns the PTY, screen model, formula detection, layout, cache, and Kitty image placements. Both backends use the same bounded request/response interface. RaTeX runs in an isolated native child process; MathJax uses a single prewarmed Node worker with direct MathJax/resvg calls. A renderer stall does not block the PTY loop; stalled workers are disabled after the existing timeout.

- Explicit delimiters: `\(…\)`, `\[…\]`, `$…$`, and `$$…$$`. Dollar inline math currently requires an operator or command.
- In compact mode, inline images use rendered width rather than LaTeX source width. Following prose moves left as native terminal text, preserving styles and Unicode.
- In compact mode, visible wrapped inline formulas are joined across at most nine source rows. The equation is placed intact on the row with the most room; other source fragments disappear. Partially visible formulas remain as text.
- One outstanding render request bounds work. Delayed results are matched to the current viewport, and cached images move without typesetting again.
- The shared image cache targets 128 images / 64 MiB of encoded PNGs, retaining currently visible images. MathJax additionally has a 16 MiB worker cache and a bounded 128 MiB checksummed disk cache. RaTeX uses its font/glyph caches and the shared session image cache; it does not currently persist final images between sessions.
- Fenced code and the detected Codex input area are excluded heuristically.

## Limits and settings

Targets Codex redraws in Ghostty, not arbitrary terminal applications or native terminal scrollback. Inline compaction does not reflow whole paragraphs. Runs of two or more source spaces are treated as alignment padding, preserving table columns while compacting math within each cell. Composer detection depends on visible prompt markers. Indexed colors fall back to configured defaults. Large equations may be scaled or left as source; TermiTex does not allocate extra rows. Terminal control coverage and cursor edge cases need broader testing.

In the default compact mode, inline compaction rewrites displayed terminal cells and moves nearby prose. Mouse coordinates are forwarded unchanged, so clicking shifted content in a mouse-driven application may target the wrong original cell. Selection/copy sees projected text, not necessarily original LaTeX. Applications using terminal state our screen model does not fully represent (such as hyperlinks or advanced text attributes) may lose that state on repainted rows. Misdetected editable text and unsupported redraw behavior can disrupt interaction. Use with arbitrary editors or TUIs is not yet validated; the underlying application data is not directly rewritten.

`TERMITEX_FG` / `TERMITEX_BG` set hex colors (defaults `#ffffff` / `#282c34`). `TERMITEX_STATS=/tmp/termitex-stats.json` writes content-free counters at exit. MathJax-only options: `TERMITEX_PREWARM=0` disables prewarming; `TERMITEX_CACHE_DIR` overrides its cache directory (`~/Library/Caches/termitex/v1` by default on macOS). Legacy `TFORMULA_*` settings are not read.

