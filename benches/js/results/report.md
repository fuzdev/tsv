# tsv benchmark results — cross-runtime

**Generated:** 2026-10-10T19:01:40.631Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.7: c93c1649 @ 2026-10-10T16:54:30.127Z (tsv 0.6.0) — corpora 63d1790f2
- `node` 24.14.1: c93c1649 @ 2026-10-10T17:36:41.639Z (tsv 0.6.0) — corpora 63d1790f2
- `bun` 1.4.2: c93c1649 @ 2026-10-10T18:20:23.079Z (tsv 0.6.0) — corpora 63d1790f2
- `conformance` (node, coverage-only): c93c1649 @ 2026-10-10T19:00:43.347Z (tsv 0.6.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 4 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `format/svelte/prettier` bun (cv 2.1%, raw 2.7%, drift -5.5%, pass spread 0.7%, n=22); `parse/typescript/oxc-parser-wasm` bun (cv 3.0%, raw 3.0%, drift +5.0%, pass spread 3.0%, n=24); `format/typescript/prettier` deno (cv 2.7%, raw 2.7%, drift -0.2%, pass spread 6.0%, n=23); `format/typescript/prettier` bun (cv 2.6%, raw 2.6%, drift -1.8%, pass spread 5.7%, n=24). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. A pass spread is how far apart the row’s passes sat — each a fresh process — so a wide one is a level that depends on the process the row was drawn in. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 4 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/tsv-wasm` deno/node (0.4% vs 1.0% noise, n=3/3); `format/typescript/oxfmt` deno/node (2.4% vs 2.7% noise, n=3/3); `parse/css/postcss` deno/node (0.6% vs 0.7% noise, n=3/3); `format/css/oxfmt` deno/bun (0.7% vs 1.7% noise, n=3/3). Read those cells as "no difference". The noise behind each side is the spread of its row's pass means (`entries[].pass_mean_ns` in the per-runtime JSON) where the row was timed in several fresh processes, and its sweep-level `cv` only where it was not — NOT that report's §Unstable Rows, which lists only rows past its own thresholds and so names none of these: a cell lands here whenever the delta is small relative to the noise. `n` counts what each side was read from — passes, or cleaned sweeps for a one-pass sibling — and a side under 3 passes or 10 sweeps is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 3.0 | 2.9 | 2.7 | 0.94x | 0.90x |
| tsv | 7.7 | 6.8 | 9.6 | 0.89x | 1.25x |
| tsv-wasm | 6.2 | 6.2 | 8.9 | 1.00x | 1.43x |
| tsv+locations | 5.4 | 5.2 | 6.8 | 0.96x | 1.25x |
| tsv-wasm+locations | 4.7 | 4.8 | 6.3 | 1.04x | 1.35x |
| tsv-internal | 62.2 | 60.5 | 69.6 | 0.97x | 1.12x |
| tsv-wasm-internal | 36.8 | 41.9 | 43.2 | 1.14x | 1.18x |
| rsvelte-parse | 1.8 | 1.7 | 2.0 | 0.95x | 1.12x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.95x | 1.08x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 ⚠ | 0.94x | 1.18x ⚠ |
| tsv | 14.9 | 14.7 | 15.0 | 0.99x | 1.01x |
| tsv-wasm | 9.2 | 10.3 | 10.6 | 1.12x | 1.14x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.95x | 1.16x |
| biome-wasm | 1.1 | 1.0 | 1.2 | 0.93x | 1.08x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 1.0 | 0.7 | 0.8 | 0.75x | 0.80x |
| tsv | 1.3 | 1.1 | 1.7 | 0.85x | 1.32x |
| tsv-wasm | 1.0 | 1.0 | 1.6 | 0.96x | 1.53x |
| tsv+locations | 1.0 | 0.9 | 1.3 | 0.89x | 1.27x |
| tsv-wasm+locations | 0.9 | 0.8 | 1.2 | 0.98x | 1.41x |
| tsv-internal | 11.0 | 9.9 | 12.4 | 0.90x | 1.13x |
| tsv-wasm-internal | 6.4 | 7.4 | 8.1 | 1.16x | 1.26x |
| oxc-parser | 0.8 | 0.7 | 1.2 | 0.89x | 1.43x |
| oxc-parser-wasm | 0.7 | 0.7 | 1.0 ⚠ | 0.94x | 1.31x ⚠ |
| yuku-parser | 2.3 | 2.6 | 3.1 | 1.15x | 1.35x |
| yuku-parser-wasm | 2.6 | 3.3 | 4.1 | 1.26x | 1.54x |
| swc | 0.6 | 0.6 | 0.7 | 0.91x | 1.20x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 ⚠ | 0.1 | 0.1 ⚠ | 0.92x ⚠ | 0.94x ⚠ |
| tsv | 2.6 | 2.5 | 2.6 | 0.95x | 1.01x |
| tsv-wasm | 1.6 | 1.8 | 1.8 | 1.13x | 1.16x |
| oxfmt | 1.2 | 1.2 | 1.2 | 0.98x | 1.02x |
| biome-wasm | 0.2 | 0.2 | 0.3 | 1.04x | 1.13x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.16x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 98.3 | 95.2 | 50.5 | 0.97x | 0.51x |
| tsv | 60.8 | 51.4 | 67.9 | 0.85x | 1.12x |
| tsv-wasm | 49.4 | 48.5 | 77.9 | 0.98x | 1.58x |
| tsv+locations | 47.9 | 43.0 | 50.9 | 0.90x | 1.06x |
| tsv-wasm+locations | 40.7 | 42.9 | 58.7 | 1.05x | 1.44x |
| tsv-internal | 396.3 | 360.2 | 430.3 | 0.91x | 1.09x |
| tsv-wasm-internal | 220.5 | 241.9 | 298.9 | 1.10x | 1.36x |
| postcss | 82.5 | 83.0 | 58.6 | 1.01x | 0.71x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.7 | 2.1 | 0.96x | 1.18x |
| tsv | 218.9 | 194.0 | 216.8 | 0.89x | 0.99x |
| tsv-wasm | 120.3 | 136.6 | 159.7 | 1.14x | 1.33x |
| oxfmt | 55.3 | 53.6 | 55.7 | 0.97x | 1.01x |
| biome-wasm | 10.6 | 11.5 | 12.4 | 1.09x | 1.17x |
| malva-wasm | 19.4 | 21.2 | 17.8 | 1.09x | 0.92x |
