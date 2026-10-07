# Comparison scope

[Back to the README](../README.md#comparison)

Reviewed October 7, 2026. This page separates recorded measurements from architectural expectations and untested comparisons.

## Evidence table

| Comparison | What was measured or checked | What it establishes | What it does not establish |
| :--- | :--- | :--- | :--- |
| Native RaTeX driver vs our MathJax worker | Five fresh processes each; 81 distinct equations; CPU, peak RSS, request latency, matched canvas dimensions | Lower native rendering cost for this workload | Current complete TermiTex performance; TFormula or TXM superiority |
| Earlier TFormula-based TermiTex backend vs our direct backend | Three local trials; request latency and sampled worker-tree RSS | An improvement over our historical integration | A benchmark of the upstream TFormula CLI; comparable peak-RSS numbers |
| Compact vs compatibility layout | Rust source-cell and placement checks; both backends exercised through a synthetic PTY | Expected behavior on those test cases | Universal terminal-app compatibility or live scrolling frame rates |
| TermiTex vs TFormula vs TXM | Upstream documentation and our implementation | Differences in workflow and features | A measured winner for CPU, memory, latency, or compatibility |

Sources: [native benchmark and raw data](../benchmarks/ratex/README.md), [historical backend measurements](../benchmarks/README.md), [Rust integration tests](../tests/), [TFormula README](https://github.com/mikewang817/TFormula#tformula), [TXM README](https://github.com/thatmagicalcat/txm#txm).

## Performance interpretation

The native driver avoids the JavaScript runtime and MathJax initialization. That is consistent with its measured lower resource use, but the benchmark does not isolate the cause of each saving. PNG encoding, layout, fonts, caching, and process startup all contribute.

TFormula documents persistent caches and reusable placements. Those can change the result for repeated formulas across sessions. Its documentation also describes sliced overlays for wrapped formulas, which our compatibility mode currently leaves unrendered. Skipping a formula must not count as a speed win. [Upstream details](https://github.com/mikewang817/TFormula#formula-and-reader-image-cache).

TXM exposes direct expression rendering and bindings. Measuring that command against a running PTY wrapper would mix different responsibilities. Its renderer could be compared separately using the same formula corpus, output geometry, and quality checks. [Upstream usage](https://github.com/thatmagicalcat/txm#txm).

For now, the supported performance claim is scoped to our recorded backend experiment. There is no matched end-to-end competitor benchmark in this repository.

## Reproducible head-to-head criteria

| Area | Test conditions | Report |
| :--- | :--- | :--- |
| Versions and machine | Pin every revision; record OS, hardware, terminal version and cell geometry | Manifest with exact reproduction commands |
| Correctness | Same valid/invalid corpus, wrapped spans, Unicode and tables; compare against unwrapped app state | Rendered, skipped and incorrect counts; image artifacts |
| Startup | Separate first launch, cold application cache and warm persistent cache | Median, p95, maximum; explain OS-cache handling |
| Steady rendering | Same stream of new and repeated formulas; matched readable output | Request and visible-display latency, total CPU |
| Memory | Include all wrapper and renderer processes; exclude the same child app and terminal consistently | Sampled aggregate RSS and measurement method; do not conflate with per-process peak RSS |
| Interaction | Scroll/resize, partial redraw, alternate screen, input, mouse, clipboard, exit status and signals | Pass/fail by case, with unwrapped baseline |
| Smoothness | Record live terminal frames under identical scroll input | Frame-time distribution, stale placements and dropped frames |

Run repeated trials in alternating order. Publish failures, workload files, and raw records as well as summary tables. A claim should name the metric and tested workload; no single table establishes compatibility with “most programs.”
