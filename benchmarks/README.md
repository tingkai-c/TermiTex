# Backend measurements

Local macOS samples, 2026-10-07; three fresh processes and fresh temporary disk caches per configuration. These measure worker request/response latency, not Ghostty presentation time. Memory is a single `ps` RSS sample summed across the worker and descendants after three requests; it excludes Rust/Codex/Ghostty, is not peak memory, and may double-count shared pages.

| Measurement | Previous backend (a19c4f7) | Single-pass backend |
|---|---:|---:|
| Immediate first request, includes process startup | 197–217 ms | 123–150 ms |
| Next uncached formula, same worker | 5.2–5.4 ms | 3.5–3.6 ms |
| Repeated formula, worker cache | 0.63–0.66 ms | 0.14–0.15 ms |
| First request after 1 second idle | 140–161 ms | 12–31 ms |
| Sampled process-tree RSS | 151–152 MiB | 83–84 MiB |
| Renderer processes | 2 | 1 |

With one second allowed for prewarming, optimized first-request latency was 12–31 ms. That excludes the deliberate one-second delay; prewarming moves initialization ahead of demand, not eliminates it. These small samples are exploratory, not general performance guarantees.

The new backend keeps MathJax, invokes resvg in the existing worker, calculates the compact canvas before rasterizing, and caches the final image. It no longer loads Sharp or starts TFormula's extra raster subprocess. Native raster errors/timeouts are contained at the existing Rust/worker process boundary. Nine formulas have pixel-by-pixel comparison coverage, alongside the PTY and Rust tests.

To repeat (requires permission to inspect processes using `ps`):

```sh
python3 benchmarks/worker.py
python3 benchmarks/worker.py worker/render.mjs 1
```

For historical comparisons, use a separate checkout at the baseline commit with its own installed dependencies. The original measurement used an extracted worker file: extract `worker/render.mjs` from commit `a19c4f7` to a temporary `.mjs` file and pass that path as the first argument. The current script supplies an isolated termitex cache directory; older backends require their own cache/dependency environment variables. `TERMITEX_PREWARM=0` can disable prewarming in the new worker. Keep formula inputs and cache state identical when comparing backends.

## Direct MathJax integration

After removing the TFormula dependency, the same three-trial test sampled 81.5–82.1 MiB RSS in one worker process. Immediate first requests were 365, 121, and 119 ms (the first cold sample was substantially slower); after one second of prewarming they were 29.8–30.1 ms. This does not establish a speed improvement over the previous single-process backend. The main change is dependency ownership: direct MathJax/resvg imports, termitex's cache, and no TFormula package or runtime configuration. All nine reference images match their pre-migration PNG hashes exactly.
