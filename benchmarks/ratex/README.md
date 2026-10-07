# RaTeX versus termitex's MathJax worker

Measured on local macOS arm64, 2026-10-07, Node v25.9.0, Rust 1.95.0. No change to the active backend or `c` launcher.

Upstream: RaTeX 0.1.14, commit `776c1d37bafa3bf445a0ab9377c55fe77f7a0133`. Native driver uses embedded fonts, full release LTO, and one codegen unit. MathJax is termitex's direct MathJax/resvg worker from `c2edd1b`, with its normal startup prewarming enabled.

Five fresh-process trials per backend, alternating order. Each trial renders 81 distinct expressions: the nine existing fixture formulas, then eight numeric variants of each. Fresh temporary disk caches for MathJax; RaTeX reuses internal font/glyph caches but has no final-image cache. No deliberate pause before the first request. OS caches were not flushed.

| Metric (median across trials) | MathJax worker | RaTeX worker | Difference |
|---|---:|---:|---:|
| CPU time for complete 81-equation process | 319.08 ms | 16.38 ms | 94.9% lower |
| Peak process RSS | 114.13 MiB | 14.84 MiB | 87.0% lower; 99.3 MiB saved |
| Warm new-equation request latency | 1.50 ms | 0.093 ms | About 16× faster |
| Launch to first PNG | 121.19 ms | 4.41 ms | Median only; see outliers below |

The first launch of each newly built executable can behave very differently: the final run includes first-response outliers of **371.8 ms for MathJax and 584.8 ms for RaTeX**. Earlier exploratory runs also had outliers. The cause was not established. Do not interpret the median as a guaranteed first-ever-launch time.

## Method and limits

- Both receive identical LaTeX, styles, colors, and requested cell sizes. They return base64 PNG responses over pipes. Output PNG dimensions match for every request (verified from PNG headers).
- RaTeX uses its own font metrics and layout. The driver adjusts font size to fit, positions inline/display drawing items, and gives the display list the same final canvas dimensions as the MathJax image. This is a benchmark adapter, not a production terminal-layout implementation.
- CPU time is child user + system time from `wait4`, including startup, typesetting, rasterization, encoding, JSON, and cache work. Peak RSS is the child's `ru_maxrss`; macOS reports bytes. The script also samples RSS with `ps` after 9 and 81 requests.
- Request latency includes pipe communication and PNG decoding in the Python harness. The parent harness and `ps` CPU are not included in child CPU time. The raw records are in `results.jsonl`.
- All 81 expressions produced PNGs in each run. Representative Maxwell, partial-derivative, chemistry, and matrix output was visually inspected. This is not a complete correctness or compatibility audit; fonts/layout differ from MathJax.
- The workload tests new formulas. It does not measure repeated final-image hits, CJK fallback, large documents, custom macros, or live Ghostty frame times. The MathJax worker also does disk-cache writes; RaTeX has no equivalent persistent cache in this harness.
- Memory numbers exclude Codex, Ghostty, and termitex's PTY/image cache. Integrating RaTeX would introduce some additional state; these numbers are not a guarantee for total app memory. Already-cached scrolling should benefit much less.

## Reproduce

Clone RaTeX separately, check out the pinned commit, then run:

```sh
python3 benchmarks/ratex/build.py /path/to/RaTeX /tmp/termitex-ratex-driver
python3 benchmarks/ratex/compare.py /tmp/termitex-ratex-driver/target/release/termitex-ratex-driver
```

The comparison needs permission to inspect its child processes using `ps`. It writes sample images to a temporary directory and prints its location on stderr. No installation into termitex is performed. Cargo.lock pins the native benchmark dependencies.

RaTeX's `render-bench` skill informed the release-build and cold/hot measurement procedure. The upstream source is MIT licensed; this directory contains only the local adapter/harness and results.
