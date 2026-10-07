# tsv benchmark results — cross-runtime

**Generated:** 2026-10-07T02:29:21.096Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.7: 3a8e53d7 @ 2026-10-07T01:52:32.975Z (tsv 0.6.0) — corpora 63d1790f2
- `node` 24.14.1: 3a8e53d7 @ 2026-10-07T02:10:58.325Z (tsv 0.6.0) — corpora 63d1790f2
- `bun` 1.4.2: 3a8e53d7 @ 2026-10-07T02:28:22.396Z (tsv 0.6.0) — corpora 63d1790f2
- `conformance` (node, coverage-only): 3a8e53d7 @ 2026-10-07T02:29:20.741Z (tsv 0.6.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 1 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/yuku-parser` deno (cv 1.9%, raw 10.8%, drift -0.2%, n=10). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 4 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `format/svelte/tsv-wasm` node/bun (0.2% vs 0.4% noise, n=40/49); `parse/css/postcss` deno/node (0.2% vs 3.1% noise, n=402/372); `format/css/oxfmt` deno/node (2.1% vs 3.3% noise, n=258/260); `format/css/oxfmt` deno/bun (1.7% vs 3.4% noise, n=258/268). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.1 | 2.0 | 0.94x | 0.90x |
| tsv | 7.1 | 6.5 | 9.4 | 0.91x | 1.32x |
| tsv-wasm | 5.9 | 5.9 | 8.7 | 1.01x | 1.47x |
| tsv+locations | 5.0 | 4.9 | 6.7 | 0.98x | 1.34x |
| tsv-wasm+locations | 4.4 | 4.6 | 6.2 | 1.05x | 1.41x |
| tsv-internal | 62.4 | 60.4 | 68.3 | 0.97x | 1.10x |
| tsv-wasm-internal | 36.0 | 41.0 | 43.0 | 1.14x | 1.19x |
| rsvelte-parse | 1.7 | 1.7 | 2.0 | 0.95x | 1.14x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.95x | 1.10x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.94x | 1.17x |
| tsv | 14.9 | 14.7 | 14.5 | 0.99x | 0.98x |
| tsv-wasm | 9.3 | 10.3 | 10.3 | 1.11x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.94x | 1.12x |
| biome-wasm | 1.1 | 0.8 | 0.9 | 0.77x | 0.86x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.3 | 0.89x | 0.83x |
| tsv | 1.1 | 1.0 | 1.6 | 0.88x | 1.43x |
| tsv-wasm | 1.0 | 1.0 | 1.6 | 0.99x | 1.63x |
| tsv+locations | 0.9 | 0.8 | 1.2 | 0.91x | 1.36x |
| tsv-wasm+locations | 0.8 | 0.8 | 1.2 | 0.99x | 1.50x |
| tsv-internal | 11.0 | 9.8 | 12.4 | 0.89x | 1.12x |
| tsv-wasm-internal | 6.4 | 7.4 | 8.0 | 1.15x | 1.26x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.42x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.97x | 1.21x |
| yuku-parser | 2.2 ⚠ | 2.5 | 3.0 | 1.12x ⚠ | 1.36x ⚠ |
| yuku-parser-wasm | 2.6 | 3.1 | 4.1 | 1.18x | 1.56x |
| swc | 0.6 | 0.5 | 0.7 | 0.93x | 1.27x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.89x | 0.87x |
| tsv | 2.7 | 2.5 | 2.6 | 0.95x | 0.97x |
| tsv-wasm | 1.6 | 1.8 | 1.8 | 1.12x | 1.13x |
| oxfmt | 1.2 | 1.2 | 1.2 | 1.00x | 1.02x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.93x | 1.03x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 77.9 | 83.7 | 49.3 | 1.07x | 0.63x |
| tsv | 54.9 | 48.4 | 70.4 | 0.88x | 1.28x |
| tsv-wasm | 45.9 | 46.8 | 77.7 | 1.02x | 1.69x |
| tsv+locations | 42.9 | 39.8 | 54.1 | 0.93x | 1.26x |
| tsv-wasm+locations | 37.3 | 38.8 | 58.8 | 1.04x | 1.58x |
| tsv-internal | 399.9 | 357.5 | 428.8 | 0.89x | 1.07x |
| tsv-wasm-internal | 215.7 | 242.0 | 298.6 | 1.12x | 1.38x |
| postcss | 80.7 | 80.9 | 61.1 | 1.00x | 0.76x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.7 | 2.0 | 0.95x | 1.12x |
| tsv | 217.1 | 192.7 | 205.3 | 0.89x | 0.95x |
| tsv-wasm | 120.5 | 136.9 | 153.4 | 1.14x | 1.27x |
| oxfmt | 54.2 | 53.1 | 55.1 | 0.98x | 1.02x |
| biome-wasm | 10.6 | 10.9 | 12.5 | 1.02x | 1.18x |
| malva-wasm | 19.4 | 21.3 | 17.7 | 1.09x | 0.91x |
