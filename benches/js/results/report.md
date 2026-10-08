# tsv benchmark results — cross-runtime

**Generated:** 2026-10-08T15:47:27.323Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.7: 3acd0be7 @ 2026-10-08T13:39:45.082Z (tsv 0.6.0) — corpora 63d1790f2
- `node` 24.14.1: 3acd0be7 @ 2026-10-08T14:22:06.374Z (tsv 0.6.0) — corpora 63d1790f2
- `bun` 1.4.2: 3acd0be7 @ 2026-10-08T15:05:59.589Z (tsv 0.6.0) — corpora 63d1790f2
- `conformance` (node, coverage-only): 3acd0be7 @ 2026-10-08T15:46:31.802Z (tsv 0.6.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 2 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/yuku-parser-wasm` bun (cv 2.4%, raw 3.0%, drift -0.4%, pass spread 5.7%, n=60); `format/typescript/prettier` bun (cv 2.7%, raw 2.7%, drift -1.1%, pass spread 6.0%, n=24). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. A pass spread is how far apart the row’s passes sat — each a fresh process — so a wide one is a level that depends on the process the row was drawn in. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 6 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/tsv-wasm` deno/node (0.1% vs 0.6% noise, n=3/3); `format/typescript/oxfmt` deno/bun (0.1% vs 1.5% noise, n=3/3); `format/typescript/oxfmt` node/bun (1.7% vs 1.7% noise, n=3/3); `format/css/oxfmt` deno/node (1.3% vs 2.1% noise, n=3/3); `format/css/oxfmt` deno/bun (0.0% vs 2.4% noise, n=3/3); `format/css/oxfmt` node/bun (1.3% vs 1.5% noise, n=3/3). Read those cells as "no difference". The noise behind each side is the spread of its row's pass means (`entries[].pass_mean_ns` in the per-runtime JSON) where the row was timed in several fresh processes, and its sweep-level `cv` only where it was not — NOT that report's §Unstable Rows, which lists only rows past its own thresholds and so names none of these: a cell lands here whenever the delta is small relative to the noise. `n` counts what each side was read from — passes, or cleaned sweeps for a one-pass sibling — and a side under 3 passes or 10 sweeps is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 3.1 | 2.8 | 2.7 | 0.93x | 0.89x |
| tsv | 7.6 | 6.9 | 9.6 | 0.90x | 1.26x |
| tsv-wasm | 6.2 | 6.2 | 8.9 | 1.00x | 1.43x |
| tsv+locations | 5.4 | 5.3 | 6.7 | 0.97x | 1.24x |
| tsv-wasm+locations | 4.6 | 4.8 | 6.4 | 1.04x | 1.37x |
| tsv-internal | 62.5 | 60.5 | 69.7 | 0.97x | 1.12x |
| tsv-wasm-internal | 36.7 | 42.0 | 43.4 | 1.14x | 1.18x |
| rsvelte-parse | 1.8 | 1.7 | 2.0 | 0.95x | 1.12x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.95x | 1.09x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.94x | 1.18x |
| tsv | 14.9 | 14.8 | 15.1 | 0.99x | 1.01x |
| tsv-wasm | 9.2 | 10.3 | 10.5 | 1.11x | 1.14x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.95x | 1.16x |
| biome-wasm | 1.1 | 1.0 | 1.2 | 0.93x | 1.08x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 1.0 | 0.7 | 0.8 | 0.74x | 0.80x |
| tsv | 1.3 | 1.1 | 1.7 | 0.85x | 1.31x |
| tsv-wasm | 1.1 | 1.0 | 1.6 | 0.96x | 1.55x |
| tsv+locations | 1.0 | 0.9 | 1.3 | 0.88x | 1.26x |
| tsv-wasm+locations | 0.9 | 0.8 | 1.2 | 0.97x | 1.41x |
| tsv-internal | 10.9 | 9.9 | 12.4 | 0.90x | 1.14x |
| tsv-wasm-internal | 6.4 | 7.4 | 8.1 | 1.16x | 1.27x |
| oxc-parser | 0.8 | 0.7 | 1.2 | 0.88x | 1.44x |
| oxc-parser-wasm | 0.7 | 0.7 | 1.0 | 0.94x | 1.31x |
| yuku-parser | 2.3 | 2.6 | 3.1 | 1.13x | 1.33x |
| yuku-parser-wasm | 2.6 | 3.4 | 4.1 ⚠ | 1.28x | 1.55x ⚠ |
| swc | 0.6 | 0.6 | 0.8 | 0.91x | 1.21x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 ⚠ | 0.92x | 0.95x ⚠ |
| tsv | 2.6 | 2.5 | 2.6 | 0.96x | 1.01x |
| tsv-wasm | 1.6 | 1.8 | 1.8 | 1.14x | 1.16x |
| oxfmt | 1.2 | 1.2 | 1.2 | 0.98x | 1.00x |
| biome-wasm | 0.2 | 0.2 | 0.3 | 1.03x | 1.13x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.17x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 98.7 | 95.4 | 51.0 | 0.97x | 0.52x |
| tsv | 61.1 | 50.9 | 67.1 | 0.83x | 1.10x |
| tsv-wasm | 49.5 | 48.6 | 77.7 | 0.98x | 1.57x |
| tsv+locations | 48.0 | 43.2 | 51.5 | 0.90x | 1.07x |
| tsv-wasm+locations | 40.9 | 42.6 | 59.0 | 1.04x | 1.44x |
| tsv-internal | 398.4 | 360.4 | 430.8 | 0.90x | 1.08x |
| tsv-wasm-internal | 218.1 | 245.4 | 302.2 | 1.13x | 1.39x |
| postcss | 80.9 | 83.6 | 59.2 | 1.03x | 0.73x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.8 | 2.2 | 0.97x | 1.20x |
| tsv | 219.0 | 193.4 | 217.9 | 0.88x | 1.00x |
| tsv-wasm | 121.0 | 136.4 | 159.3 | 1.13x | 1.32x |
| oxfmt | 55.2 | 54.5 | 55.2 | 0.99x | 1.00x |
| biome-wasm | 10.6 | 11.6 | 12.4 | 1.09x | 1.17x |
| malva-wasm | 19.5 | 21.3 | 18.3 | 1.09x | 0.94x |
