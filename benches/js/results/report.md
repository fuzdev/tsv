# tsv benchmark results — cross-runtime

**Generated:** 2026-10-18T07:37:54.808Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.7: 7103b4aa @ 2026-10-18T04:22:30.162Z (tsv 0.7.0) — corpora 976ba903d
- `node` 24.14.1: 7103b4aa @ 2026-10-18T05:27:37.267Z (tsv 0.7.0) — corpora 976ba903d
- `bun` 1.4.2: 7103b4aa @ 2026-10-18T06:35:57.760Z (tsv 0.7.0) — corpora 976ba903d
- `conformance` (node, coverage-only): 7103b4aa @ 2026-10-18T07:36:58.821Z (tsv 0.7.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 1 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/swc` node (cv 5.9%, raw 5.9%, drift +0.1%, pass spread 12.7%, n=23). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. A pass spread is how far apart the row’s passes sat — each a fresh process — so a wide one is a level that depends on the process the row was drawn in. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 5 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/svelte/compiler` node/bun (1.3% vs 1.6% noise, n=3/3); `format/typescript/oxfmt` deno/node (0.3% vs 1.4% noise, n=3/3); `format/typescript/oxfmt` deno/bun (1.5% vs 1.8% noise, n=3/3); `parse/css/tsv-wasm` deno/node (0.9% vs 1.1% noise, n=3/3); `format/css/oxfmt` deno/bun (0.2% vs 1.0% noise, n=3/3). Read those cells as "no difference". The noise behind each side is the spread of its row's pass means (`entries[].pass_mean_ns` in the per-runtime JSON) where the row was timed in several fresh processes, and its sweep-level `cv` only where it was not — NOT that report's §Unstable Rows, which lists only rows past its own thresholds and so names none of these: a cell lands here whenever the delta is small relative to the noise. `n` counts what each side was read from — passes, or cleaned sweeps for a one-pass sibling — and a side under 3 passes or 10 sweeps is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.4 | 1.3 | 1.2 | 0.91x | 0.90x |
| tsv | 3.7 | 3.3 | 4.9 | 0.90x | 1.32x |
| tsv-wasm | 3.0 | 3.0 | 4.4 | 1.00x | 1.46x |
| tsv+locations | 2.6 | 2.5 | 3.4 | 0.96x | 1.29x |
| tsv-wasm+locations | 2.2 | 2.3 | 3.1 | 1.04x | 1.39x |
| tsv-internal | 31.3 | 30.7 | 34.2 | 0.98x | 1.09x |
| tsv-wasm-internal | 17.8 | 19.9 | 20.7 | 1.12x | 1.17x |
| rsvelte-parse | 0.9 | 0.8 | 1.0 | 0.94x | 1.15x |
| rsvelte-parse-skip-expr-loc | 1.4 | 1.3 | 1.6 | 0.94x | 1.11x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.95x | 1.17x |
| tsv | 7.4 | 7.4 | 7.5 | 1.00x | 1.01x |
| tsv-wasm | 4.5 | 5.1 | 5.2 | 1.14x | 1.16x |
| oxfmt | 0.1 | 0.1 | 0.1 | 0.95x | 1.15x |
| biome-wasm | 0.5 | 0.5 | 0.6 | 0.92x | 1.07x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.5 | 0.4 | 0.4 | 0.77x | 0.81x |
| tsv | 0.7 | 0.6 | 0.9 | 0.86x | 1.32x |
| tsv-wasm | 0.6 | 0.5 | 0.9 | 0.97x | 1.53x |
| tsv+locations | 0.5 | 0.5 | 0.7 | 0.90x | 1.24x |
| tsv-wasm+locations | 0.5 | 0.5 | 0.6 | 0.98x | 1.37x |
| tsv-internal | 6.0 | 5.6 | 6.7 | 0.92x | 1.11x |
| tsv-wasm-internal | 3.3 | 3.8 | 4.1 | 1.14x | 1.24x |
| oxc-parser | 0.4 | 0.4 | 0.6 | 0.90x | 1.45x |
| oxc-parser-wasm | 0.4 | 0.4 | 0.5 | 0.95x | 1.29x |
| yuku-parser | 1.2 | 1.4 | 1.7 | 1.16x | 1.38x |
| yuku-parser-wasm | 1.5 | 1.8 | 2.2 | 1.25x | 1.51x |
| swc | 0.3 | 0.3 ⚠ | 0.4 | 0.88x ⚠ | 1.22x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.0 | 0.0 | 0.0 | 0.91x | 0.97x |
| tsv | 1.4 | 1.4 | 1.4 | 0.95x | 0.99x |
| tsv-wasm | 0.8 | 1.0 | 1.0 | 1.13x | 1.14x |
| oxfmt | 0.6 | 0.6 | 0.6 | 1.00x | 1.01x |
| biome-wasm | 0.1 | 0.1 | 0.1 | 1.03x | 1.13x |
| dprint-wasm | 0.1 | 0.2 | 0.2 | 1.13x | 1.17x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 79.3 | 76.9 | 42.3 | 0.97x | 0.53x |
| tsv | 45.8 | 39.8 | 50.5 | 0.87x | 1.10x |
| tsv-wasm | 37.8 | 37.4 | 60.1 | 0.99x | 1.59x |
| tsv+locations | 36.1 | 33.7 | 39.2 | 0.93x | 1.09x |
| tsv-wasm+locations | 30.8 | 32.3 | 46.0 | 1.05x | 1.49x |
| tsv-internal | 318.4 | 298.3 | 363.1 | 0.94x | 1.14x |
| tsv-wasm-internal | 179.6 | 201.0 | 246.8 | 1.12x | 1.37x |
| postcss | 67.0 | 65.7 | 46.7 | 0.98x | 0.70x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.4 | 1.4 | 1.7 | 0.96x | 1.18x |
| tsv | 174.9 | 157.4 | 179.0 | 0.90x | 1.02x |
| tsv-wasm | 97.9 | 110.8 | 131.2 | 1.13x | 1.34x |
| oxfmt | 43.9 | 43.0 | 43.9 | 0.98x | 1.00x |
| biome-wasm | 8.3 | 9.0 | 9.9 | 1.08x | 1.19x |
| malva-wasm | 15.3 | 16.6 | 14.3 | 1.09x | 0.94x |
