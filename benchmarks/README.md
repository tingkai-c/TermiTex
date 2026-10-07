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

To compare the old pipeline, extract `worker/render.mjs` from commit `a19c4f7` to a temporary `.mjs` file and pass that path as the first argument. The script supplies the dependency path and an isolated cache directory. `TERMITEX_PREWARM=0` can disable prewarming in the new worker. Keep formula inputs and cache state identical when comparing backends.
