# tsv benchmark results — cross-runtime

**Generated:** 2026-09-19T23:05:48.861Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 8837350f @ 2026-09-19T22:27:22.227Z (tsv 0.4.1) — corpora b4c8d2862
- `node` 24.14.1: 8837350f @ 2026-09-19T22:46:13.085Z (tsv 0.4.1) — corpora b4c8d2862
- `bun` 1.4.2: 8837350f @ 2026-09-19T23:04:29.425Z (tsv 0.4.1) — corpora b4c8d2862
- `conformance` (node, coverage-only): 8837350f @ 2026-09-19T23:05:48.534Z (tsv 0.4.1)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 1 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/svelte/svelte/compiler` bun (cv 3.3%, raw 3.3%, drift -6.1%, n=16). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 4 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/css/postcss` deno/node (1.4% vs 3.0% noise, n=402/333); `format/css/oxfmt` deno/node (2.6% vs 3.9% noise, n=265/260); `format/css/oxfmt` deno/bun (0.4% vs 3.8% noise, n=265/264); `format/css/oxfmt` node/bun (3.1% vs 4.2% noise, n=260/264). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.6 | 1.6 | 1.3 ⚠ | 0.96x | 0.76x ⚠ |
| tsv-json | 4.2 | 3.7 | 6.2 | 0.89x | 1.49x |
| tsv-wasm-json | 3.5 | 3.5 | 6.0 | 0.99x | 1.71x |
| tsv-json-no-locations | 6.8 | 6.2 | 8.9 | 0.91x | 1.31x |
| tsv-wasm-json-no-locations | 5.4 | 5.6 | 8.1 | 1.02x | 1.50x |
| tsv-internal | 50.5 | 49.3 | 54.9 | 0.98x | 1.09x |
| tsv-wasm-internal | 29.6 | 33.9 | 35.8 | 1.15x | 1.21x |
| rsvelte-parse | 1.8 | 1.7 | 2.1 | 0.95x | 1.15x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.1 | 0.96x | 1.12x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.97x | 1.20x |
| tsv | 14.0 | 13.8 | 13.7 | 0.99x | 0.98x |
| tsv-wasm | 8.4 | 9.7 | 9.5 | 1.16x | 1.13x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 1.12x |
| biome-wasm | 1.1 | 0.9 | 0.9 | 0.79x | 0.86x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.92x | 0.62x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.69x |
| tsv-wasm-json | 0.5 | 0.5 | 0.9 | 0.96x | 1.92x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.6 | 0.89x | 1.40x |
| tsv-wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 1.00x | 1.62x |
| tsv-internal | 9.6 | 8.8 | 10.7 | 0.92x | 1.12x |
| tsv-wasm-internal | 5.8 | 6.8 | 7.4 | 1.17x | 1.27x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.91x | 1.45x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.96x | 1.20x |
| yuku-parser | 2.1 | 2.4 | 2.9 | 1.10x | 1.37x |
| yuku-parser-wasm | 2.3 | 2.7 | 3.5 | 1.18x | 1.53x |
| swc | 0.6 | 0.6 | 0.7 | 0.95x | 1.25x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.89x | 0.85x |
| tsv | 2.5 | 2.4 | 2.5 | 0.96x | 0.98x |
| tsv-wasm | 1.5 | 1.8 | 1.7 | 1.19x | 1.17x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.99x | 1.00x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.94x | 1.05x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.15x | 1.17x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 75.9 | 81.5 | 48.9 | 1.07x | 0.64x |
| tsv-json | 51.1 | 45.8 | 64.0 | 0.90x | 1.25x |
| tsv-wasm-json | 41.6 | 43.2 | 67.1 | 1.04x | 1.61x |
| tsv-internal | 276.5 | 255.3 | 289.2 | 0.92x | 1.05x |
| tsv-wasm-internal | 152.0 | 171.8 | 197.1 | 1.13x | 1.30x |
| postcss | 80.7 | 81.8 | 61.3 | 1.01x | 0.76x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.7 | 2.0 | 0.95x | 1.13x |
| tsv | 161.1 | 149.6 | 155.1 | 0.93x | 0.96x |
| tsv-wasm | 89.6 | 104.5 | 112.0 | 1.17x | 1.25x |
| oxfmt | 54.0 | 52.5 | 54.2 | 0.97x | 1.00x |
| biome-wasm | 10.2 | 10.4 | 12.2 | 1.03x | 1.20x |
| malva-wasm | 19.4 | 21.1 | 18.2 | 1.09x | 0.94x |
