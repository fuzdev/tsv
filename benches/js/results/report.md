# tsv benchmark results — cross-runtime

**Generated:** 2026-09-15T17:48:41.268Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 29a107e9 @ 2026-09-15T17:17:31.122Z (tsv 0.3.0) — corpora e8d37ffc1
- `node` 24.14.1: 29a107e9 @ 2026-09-15T17:33:04.436Z (tsv 0.3.0) — corpora e8d37ffc1
- `bun` 1.4.2: 29a107e9 @ 2026-09-15T17:47:22.394Z (tsv 0.3.0) — corpora e8d37ffc1
- `conformance` (node, coverage-only): 29a107e9 @ 2026-09-15T17:48:40.955Z (tsv 0.3.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 1 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/oxc-parser-wasm` bun (cv 3.3%, raw 3.3%, drift +6.2%, n=8). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 4 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `format/svelte/tsv` deno/node (0.2% vs 0.6% noise, n=63/60); `parse/css/postcss` deno/node (1.5% vs 2.9% noise, n=410/317); `format/css/oxfmt` deno/node (1.8% vs 3.9% noise, n=240/243); `format/css/oxfmt` node/bun (2.4% vs 4.3% noise, n=243/239). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.7 | 1.6 | 1.2 | 0.97x | 0.74x |
| tsv-json | 4.1 | 3.7 | 6.1 | 0.89x | 1.47x |
| tsv_wasm-json | 3.5 | 3.4 | 5.9 | 0.98x | 1.67x |
| tsv-json-no-locations | 6.7 | 6.1 | 8.4 | 0.91x | 1.26x |
| tsv_wasm-json-no-locations | 5.4 | 5.4 | 7.8 | 1.01x | 1.45x |
| tsv-internal | 47.4 | 45.6 | 50.4 | 0.96x | 1.06x |
| tsv_wasm-internal | 28.7 | 32.4 | 32.9 | 1.13x | 1.15x |
| rsvelte-parse | 1.8 | 1.7 | 2.1 | 0.94x | 1.16x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.1 | 0.95x | 1.11x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.93x | 1.23x |
| tsv | 12.7 | 12.7 | 12.5 | 1.00x | 0.99x |
| tsv_wasm | 7.9 | 9.1 | 8.8 | 1.15x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.95x | 1.16x |
| biome-wasm | 1.1 | 0.9 | 1.0 | 0.80x | 0.88x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.93x | 0.62x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.66x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.96x | 1.87x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.5 | 0.88x | 1.38x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.99x | 1.58x |
| tsv-internal | 9.1 | 8.4 | 10.0 | 0.92x | 1.10x |
| tsv_wasm-internal | 5.7 | 6.6 | 6.9 | 1.14x | 1.21x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.89x | 1.44x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 ⚠ | 0.95x | 1.21x ⚠ |
| yuku-parser | 2.1 | 2.3 | 2.9 | 1.11x | 1.39x |
| yuku-parser-wasm | 2.3 | 2.7 | 3.6 | 1.15x | 1.52x |
| swc | 0.6 | 0.5 | 0.7 | 0.94x | 1.29x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.89x | 0.98x |
| tsv | 2.3 | 2.2 | 2.2 | 0.97x | 0.97x |
| tsv_wasm | 1.4 | 1.6 | 1.6 | 1.17x | 1.12x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.98x | 0.99x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.95x | 1.07x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.16x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 91.9 | 88.1 | 50.1 | 0.96x | 0.55x |
| tsv-json | 49.2 | 44.4 | 61.2 | 0.90x | 1.24x |
| tsv_wasm-json | 39.8 | 41.4 | 60.8 | 1.04x | 1.53x |
| tsv-internal | 229.1 | 215.3 | 239.7 | 0.94x | 1.05x |
| tsv_wasm-internal | 128.8 | 146.4 | 153.4 | 1.14x | 1.19x |
| postcss | 83.1 | 81.9 | 72.6 | 0.99x | 0.87x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.6 | 1.5 | 2.1 | 0.92x | 1.26x |
| tsv | 131.7 | 122.7 | 126.6 | 0.93x | 0.96x |
| tsv_wasm | 74.2 | 85.7 | 87.9 | 1.15x | 1.18x |
| oxfmt | 47.9 | 48.7 | 49.9 | 1.02x | 1.04x |
| biome-wasm | 9.8 | 10.1 | 11.6 | 1.02x | 1.18x |
| malva-wasm | 17.5 | 19.2 | 16.6 | 1.10x | 0.95x |
