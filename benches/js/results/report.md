# tsv benchmark results — cross-runtime

**Generated:** 2026-11-02T07:43:00.644Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.8: 83aa5582 @ 2026-11-02T04:25:40.349Z (tsv 0.8.0) — corpora 678f4942f
- `node` 24.14.1: 83aa5582 @ 2026-11-02T05:32:15.271Z (tsv 0.8.0) — corpora 678f4942f
- `bun` 1.4.4: 83aa5582 @ 2026-11-02T06:41:43.538Z (tsv 0.8.0) — corpora 678f4942f
- `conformance` (node, coverage-only): 83aa5582 @ 2026-11-02T07:41:52.090Z (tsv 0.8.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — oxc-parser-raw. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Unstable:** 4 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/oxc-parser-raw` node (cv 4.5%, raw 5.0%, drift +4.8%, pass spread 8.5%, n=20); `parse/typescript/swc` node (cv 4.9%, raw 4.9%, drift -0.2%, pass spread 10.5%, n=24); `format/typescript/oxfmt` bun (cv 1.9%, raw 1.9%, drift +5.8%, pass spread 1.6%, n=24); `parse/css/tsv-wasm+locations` bun (cv 3.2%, raw 3.3%, drift -2.7%, pass spread 5.9%, n=739). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. A pass spread is how far apart the row’s passes sat — each a fresh process — so a wide one is a level that depends on the process the row was drawn in. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 5 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/tsv-wasm` deno/node (0.4% vs 0.8% noise, n=3/3); `format/typescript/prettier` deno/bun (0.3% vs 3.2% noise, n=3/3); `parse/css/tsv-wasm` deno/node (1.8% vs 2.0% noise, n=3/3); `parse/css/postcss` deno/node (0.2% vs 1.1% noise, n=3/3); `format/css/oxfmt` deno/bun (0.2% vs 0.7% noise, n=3/3). Read those cells as "no difference". The noise behind each side is the spread of its row's pass means (`entries[].pass_mean_ns` in the per-runtime JSON) where the row was timed in several fresh processes, and its sweep-level `cv` only where it was not — NOT that report's §Unstable Rows, which lists only rows past its own thresholds and so names none of these: a cell lands here whenever the delta is small relative to the noise. `n` counts what each side was read from — passes, or cleaned sweeps for a one-pass sibling — and a side under 3 passes or 10 sweeps is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.4 | 1.3 | 1.3 | 0.93x | 0.91x |
| tsv | 3.7 | 3.4 | 5.9 | 0.91x | 1.61x |
| tsv-wasm | 3.0 | 3.0 | 5.0 | 1.00x | 1.69x |
| tsv+locations | 2.6 | 2.5 | 3.9 | 0.97x | 1.48x |
| tsv-wasm+locations | 2.2 | 2.3 | 3.4 | 1.05x | 1.55x |
| tsv-internal | 30.3 | 29.3 | 32.4 | 0.97x | 1.07x |
| tsv-wasm-internal | 17.0 | 19.1 | 20.0 | 1.13x | 1.18x |
| rsvelte-parse | 0.9 | 0.8 | 1.1 | 0.94x | 1.24x |
| rsvelte-parse-skip-expr-loc | 1.4 | 1.4 | 1.7 | 0.95x | 1.21x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.95x | 1.16x |
| tsv | 7.3 | 7.2 | 7.3 | 0.99x | 1.01x |
| tsv-wasm | 4.3 | 5.0 | 5.1 | 1.15x | 1.18x |
| oxfmt | 0.1 | 0.1 | 0.1 | 0.95x | 1.14x |
| biome-wasm | 0.5 | 0.5 | 0.5 | 0.89x | 1.05x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.5 | 0.4 | 0.4 | 0.76x | 0.84x |
| tsv | 0.7 | 0.6 | 1.2 | 0.85x | 1.83x |
| tsv-wasm | 0.6 | 0.5 | 1.0 | 0.97x | 1.86x |
| tsv+locations | 0.5 | 0.5 | 0.8 | 0.90x | 1.59x |
| tsv-wasm+locations | 0.5 | 0.4 | 0.7 | 0.99x | 1.62x |
| tsv-internal | 5.8 | 5.2 | 6.2 | 0.90x | 1.07x |
| tsv-wasm-internal | 3.2 | 3.6 | 3.9 | 1.13x | 1.24x |
| oxc-parser | 0.4 | 0.4 | 0.9 | 0.89x | 1.95x |
| oxc-parser-wasm | 0.4 | 0.4 | 0.6 | 0.95x | 1.47x |
| oxc-parser-raw | 1.6 | 1.5 ⚠ | — | 0.95x ⚠ | — |
| yuku-parser | 1.3 | 1.5 | 1.8 | 1.15x | 1.37x |
| yuku-parser-wasm | 1.5 | 1.9 | 2.3 | 1.30x | 1.52x |
| swc | 0.3 | 0.3 ⚠ | 0.5 | 0.88x ⚠ | 1.55x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.0 | 0.0 | 0.0 | 0.94x | 1.00x |
| tsv | 1.4 | 1.3 | 1.4 | 0.96x | 1.01x |
| tsv-wasm | 0.8 | 0.9 | 0.9 | 1.15x | 1.16x |
| oxfmt | 0.7 | 0.7 | 0.7 ⚠ | 0.98x | 1.02x ⚠ |
| biome-wasm | 0.1 | 0.1 | 0.1 | 0.98x | 1.11x |
| dprint-wasm | 0.1 | 0.2 | 0.2 | 1.13x | 1.17x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 79.1 | 77.0 | 40.8 | 0.97x | 0.52x |
| tsv | 46.7 | 39.1 | 79.0 | 0.84x | 1.69x |
| tsv-wasm | 38.6 | 37.9 | 73.2 | 0.98x | 1.90x |
| tsv+locations | 36.3 | 33.7 | 56.0 | 0.93x | 1.54x |
| tsv-wasm+locations | 31.3 | 32.5 | 54.1 ⚠ | 1.04x | 1.73x ⚠ |
| tsv-internal | 309.2 | 292.0 | 354.2 | 0.94x | 1.15x |
| tsv-wasm-internal | 178.2 | 202.2 | 248.4 | 1.13x | 1.39x |
| postcss | 65.3 | 65.2 | 47.3 | 1.00x | 0.72x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.5 | 1.4 | 1.8 | 0.97x | 1.20x |
| tsv | 174.0 | 158.1 | 185.2 | 0.91x | 1.06x |
| tsv-wasm | 99.7 | 113.1 | 133.2 | 1.13x | 1.34x |
| oxfmt | 46.2 | 44.7 | 46.0 | 0.97x | 1.00x |
| biome-wasm | 9.8 | 10.3 | 11.7 | 1.06x | 1.20x |
| malva-wasm | 15.2 | 16.6 | 14.2 | 1.09x | 0.93x |
