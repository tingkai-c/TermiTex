# Rendering reference

[Back to the README](../README.md)

## Design

Rust owns the PTY, screen model, formula detection, layout, cache, and Kitty image placements. Both backends use the same bounded request/response interface. The renderer pool uses two isolated native RaTeX processes or two Node workers with direct MathJax/resvg calls. Each has at most one request in flight; ready results wake the PTY loop and are applied together. A renderer stall does not block the PTY loop; stalled workers are disabled after the existing timeout.

- Explicit delimiters: `\(…\)`, `\[…\]`, `$…$`, and `$$…$$`. Inline dollars follow [Pandoc-style boundaries](https://pandoc.org/MANUAL.html#math): no whitespace immediately after the opening `$` or before the closing `$`, and no digit immediately after the closing `$`. Escaped delimiters stay literal; simple formulas such as `$x$` are supported. Dollar runs must match exactly, and `$$` display blocks cannot contain blank lines.
- In compact mode, inline images use rendered width rather than LaTeX source width. Following prose moves left as native terminal text, preserving styles and Unicode.
- In compact mode, visible wrapped inline formulas are joined across at most nine source rows. Single-dollar formulas require terminal soft wraps; `\(…\)` also supports explicit row positioning used by coding CLIs. The equation is placed intact on the row with the most room; other source fragments disappear. Partially visible formulas remain as text.
- At most two outstanding render requests bound work. Delayed results are matched to the current viewport, and cached images move without typesetting again.
- The shared image cache targets 128 images / 64 MiB of encoded PNGs, retaining currently visible images. Each MathJax worker additionally has a 16 MiB memory cache and a bounded 128 MiB checksummed disk cache. RaTeX uses its font/glyph caches and the shared session image cache; it does not currently persist final images between sessions.
- Fenced code is excluded. App-specific policies exclude detected Codex and Claude Code input areas, including multiline drafts. Policies are selected by executable basename (`codex` or `claude`); generic commands have no app-specific input exclusion.

## Limits and settings

Terminal graphics are auto-detected through a shared Kitty graphics backend. See [terminal setup and validation status](terminals.md). The wrapper propagates the child program's exit code (or 128 + signal number).

Compatibility mode preserves source cells and centers overlays. Wrapped inline formulas remain readable LaTeX in this mode to avoid covering neighboring prose. Input-area and code detection remain heuristic in both layouts.

Only the selected renderer starts. There is no automatic fallback between RaTeX and MathJax: unsupported or oversized formulas remain source text. Their appearance and extension coverage differ.

Homebrew builds from a pinned release and installs Rust as a build dependency; the first installation may take a few minutes. The package uses the project's own tap and includes the native renderer. MathJax requires a source checkout with Node and npm dependencies.

Codex redraws have the broadest testing. Claude Code input exclusion is covered by synthetic screen fixtures; other layouts and native terminal scrollback require broader testing. Inline compaction does not reflow whole paragraphs. Runs of two or more source spaces are treated as alignment padding, preserving table columns while compacting math within each cell. Composer detection depends on visible prompt markers. Indexed colors fall back to configured defaults. Large equations may be scaled or left as source; TermiTex does not allocate extra rows. Terminal control coverage and cursor edge cases need broader testing.

In the default compact mode, inline compaction rewrites displayed terminal cells and moves nearby prose. Mouse coordinates are forwarded unchanged, so clicking shifted content in a mouse-driven application may target the wrong original cell. Selection/copy sees projected text, not necessarily original LaTeX. Applications using terminal state our screen model does not fully represent (such as hyperlinks or advanced text attributes) may lose that state on repainted rows. Misdetected editable text and unsupported redraw behavior can disrupt interaction. Use with arbitrary editors or TUIs is not yet validated; the underlying application data is not directly rewritten.

`TERMITEX_FG` / `TERMITEX_BG` override queried terminal colors (fallbacks `#ffffff` / `#282c34` if the terminal does not answer). `TERMITEX_STATS=/tmp/termitex-stats.json` writes content-free counters at exit. MathJax-only options: `TERMITEX_PREWARM=0` disables prewarming; `TERMITEX_CACHE_DIR` overrides its cache directory (`~/Library/Caches/termitex/v1` by default on macOS). Legacy `TFORMULA_*` settings are not read.


## Command line and startup

Use `termitex [options] <app> [app arguments...]`. Wrapper options must precede the app; later flags belong to the child. `termitex codex` adds `-c tui.rendering.math=false` before the supplied app arguments. The explicit `--` separator preserves literal child arguments, including for Codex. Without an app, TermiTex shows brief usage and exits successfully without launching a child or probing the terminal.

The app starts before the terminal capability probe, allowing app loading to overlap its bounded 500 ms negotiation window. Initial output stays in the PTY's bounded kernel buffer; a prolific child may temporarily block until negotiation ends. Terminal replies are filtered before keyboard input reaches the child. Window size is refreshed before releasing that input; resizing interactive apps receive SIGWINCH. This overlaps startup work but does not eliminate the maximum probe delay before first visible output. `doctor` still probes without starting an app.
