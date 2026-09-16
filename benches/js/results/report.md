# tsv benchmark results — cross-runtime

**Generated:** 2026-09-16T20:52:08.837Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 0d7c0251 @ 2026-09-16T20:20:39.043Z (tsv 0.4.0) — corpora e8d37ffc1
- `node` 24.14.1: 0d7c0251 @ 2026-09-16T20:36:06.121Z (tsv 0.4.0) — corpora e8d37ffc1
- `bun` 1.4.2: 0d7c0251 @ 2026-09-16T20:50:50.310Z (tsv 0.4.0) — corpora e8d37ffc1
- `conformance` (node, coverage-only): 0d7c0251 @ 2026-09-16T20:52:08.514Z (tsv 0.4.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Within noise:** 6 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/css/svelte/compiler` deno/node (1.9% vs 3.1% noise, n=451/396); `parse/css/postcss` deno/node (0.4% vs 3.2% noise, n=404/365); `format/css/oxfmt` deno/node (1.1% vs 3.7% noise, n=238/238); `format/css/oxfmt` deno/bun (2.8% vs 3.7% noise, n=238/241); `format/css/oxfmt` node/bun (3.9% vs 4.1% noise, n=238/241); `format/css/biome-wasm` deno/node (2.7% vs 6.6% noise, n=45/45). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.7 | 1.6 | 1.2 | 0.96x | 0.74x |
| tsv-json | 4.1 | 3.8 | 6.0 | 0.91x | 1.46x |
| tsv-wasm-json | 3.6 | 3.5 | 5.8 | 0.99x | 1.64x |
| tsv-json-no-locations | 6.7 | 6.2 | 8.5 | 0.91x | 1.26x |
| tsv-wasm-json-no-locations | 5.4 | 5.5 | 7.9 | 1.02x | 1.46x |
| tsv-internal | 48.5 | 47.2 | 52.2 | 0.97x | 1.08x |
| tsv-wasm-internal | 29.3 | 32.6 | 33.8 | 1.11x | 1.16x |
| rsvelte-parse | 1.8 | 1.7 | 2.1 | 0.95x | 1.14x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.95x | 1.11x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.97x | 1.20x |
| tsv | 12.9 | 12.7 | 12.5 | 0.99x | 0.97x |
| tsv-wasm | 8.0 | 9.1 | 8.8 | 1.14x | 1.10x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.95x | 1.13x |
| biome-wasm | 1.1 | 0.9 | 0.9 | 0.78x | 0.87x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.92x | 0.60x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.89x | 1.67x |
| tsv-wasm-json | 0.5 | 0.5 | 0.9 | 0.98x | 1.91x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.5 | 0.88x | 1.39x |
| tsv-wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.99x | 1.59x |
| tsv-internal | 9.1 | 8.4 | 10.2 | 0.92x | 1.12x |
| tsv-wasm-internal | 5.7 | 6.5 | 7.1 | 1.16x | 1.25x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.42x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.96x | 1.21x |
| yuku-parser | 2.1 | 2.4 | 2.9 | 1.13x | 1.37x |
| yuku-parser-wasm | 2.3 | 2.7 | 3.6 | 1.19x | 1.55x |
| swc | 0.6 | 0.6 | 0.7 | 0.95x | 1.29x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.89x | 0.87x |
| tsv | 2.3 | 2.2 | 2.2 | 0.97x | 0.98x |
| tsv-wasm | 1.4 | 1.6 | 1.6 | 1.16x | 1.14x |
| oxfmt | 1.1 | 1.1 | 1.1 | 1.00x | 1.01x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.93x | 1.03x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 90.1 | 88.4 | 49.5 | 0.98x | 0.55x |
| tsv-json | 50.1 | 45.3 | 63.7 | 0.90x | 1.27x |
| tsv-wasm-json | 41.1 | 43.1 | 65.7 | 1.05x | 1.60x |
| tsv-internal | 265.4 | 256.2 | 289.0 | 0.97x | 1.09x |
| tsv-wasm-internal | 149.3 | 168.5 | 189.2 | 1.13x | 1.27x |
| postcss | 81.0 | 81.4 | 62.4 | 1.00x | 0.77x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.6 | 1.6 | 1.9 | 0.98x | 1.18x |
| tsv | 143.6 | 133.0 | 138.6 | 0.93x | 0.96x |
| tsv-wasm | 80.1 | 93.7 | 99.2 | 1.17x | 1.24x |
| oxfmt | 48.3 | 47.8 | 49.7 | 0.99x | 1.03x |
| biome-wasm | 9.8 | 9.5 | 11.7 | 0.97x | 1.20x |
| malva-wasm | 17.6 | 19.1 | 16.4 | 1.09x | 0.93x |
