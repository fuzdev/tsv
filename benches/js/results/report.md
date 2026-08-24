# tsv benchmark results — cross-runtime

**Generated:** 2026-08-24T15:48:48.512Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.5: 831e5193 @ 2026-08-24T15:28:09.910Z (tsv 0.2.0)
- `node` 24.14.1: 831e5193 @ 2026-08-24T15:38:47.288Z (tsv 0.2.0)
- `bun` 1.4.0: 831e5193 @ 2026-08-24T15:47:31.275Z (tsv 0.2.0)
- `conformance` (node, coverage-only): 831e5193 @ 2026-08-24T15:48:48.215Z (tsv 0.2.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — biome-wasm. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Within noise:** 9 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/svelte/compiler` node (0.9% vs 3.8% noise, n=11/10); `parse/svelte/tsv_wasm-json` node (0.2% vs 1.2% noise, n=22/18); `parse/svelte/tsv_wasm-json-no-locations` node (1.6% vs 1.8% noise, n=32/34); `format/svelte/tsv` node (1.2% vs 2.1% noise, n=60/54); `format/svelte/tsv` bun (0.3% vs 2.3% noise, n=60/63); `parse/css/svelte/compiler` node (3.2% vs 5.8% noise, n=547/494); `parse/css/postcss` node (0.3% vs 4.7% noise, n=486/482); `format/css/oxfmt` node (0.2% vs 4.4% noise, n=288/287); `format/css/oxfmt` bun (0.3% vs 4.7% noise, n=288/284). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.1 | 2.0 | 1.6 | 0.99x | 0.77x |
| tsv-json | 5.2 | 4.6 | 7.0 | 0.89x | 1.35x |
| tsv_wasm-json | 4.4 | 4.4 | 6.7 | 1.00x | 1.52x |
| tsv-json-no-locations | 8.1 | 7.5 | 9.5 | 0.92x | 1.17x |
| tsv_wasm-json-no-locations | 6.6 | 6.7 | 8.9 | 1.02x | 1.35x |
| tsv-internal | 50.1 | 49.0 | 52.6 | 0.98x | 1.05x |
| tsv_wasm-internal | 32.0 | 34.8 | 36.3 | 1.09x | 1.14x |
| rsvelte-parse | 2.9 | 2.7 | 3.3 | 0.93x | 1.14x |
| rsvelte-parse-skip-expr-loc | 4.8 | 4.5 | 5.2 | 0.94x | 1.08x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.97x | 1.27x |
| tsv | 13.0 | 13.2 | 13.0 | 1.01x | 1.00x |
| tsv_wasm | 8.5 | 9.4 | 9.4 | 1.10x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.3 | 0.97x | 1.19x |
| biome-wasm | 1.3 | 1.1 | — | 0.81x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.94x | 0.64x |
| tsv-json | 0.5 | 0.5 | 0.8 | 0.88x | 1.53x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.96x | 1.77x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.4 | 0.89x | 1.31x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.4 | 0.99x | 1.50x |
| tsv-internal | 7.5 | 7.0 | 8.1 | 0.94x | 1.07x |
| tsv_wasm-internal | 5.0 | 5.5 | 5.8 | 1.09x | 1.16x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.40x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.8 | 0.96x | 1.13x |
| yuku-parser | 2.2 | 2.4 | 3.0 | 1.09x | 1.34x |
| yuku-parser-wasm | 2.5 | 2.9 | 3.7 | 1.18x | 1.51x |
| swc | 0.6 | 0.6 | 0.7 | 0.96x | 1.24x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.92x |
| tsv | 1.8 | 1.7 | 1.7 | 0.98x | 0.99x |
| tsv_wasm | 1.1 | 1.3 | 1.3 | 1.12x | 1.14x |
| oxfmt | 1.2 | 1.1 | 1.2 | 0.99x | 1.00x |
| biome-wasm | 0.2 | 0.2 | — | 0.94x | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 109.9 | 106.4 | 65.1 | 0.97x | 0.59x |
| tsv-json | 63.8 | 55.6 | 71.8 | 0.87x | 1.13x |
| tsv_wasm-json | 51.2 | 53.2 | 74.2 | 1.04x | 1.45x |
| tsv-internal | 311.5 | 290.9 | 329.7 | 0.93x | 1.06x |
| tsv_wasm-internal | 173.9 | 204.8 | 223.8 | 1.18x | 1.29x |
| postcss | 99.3 | 99.6 | 85.9 | 1.00x | 0.87x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 2.0 | 1.8 | 2.5 | 0.91x | 1.30x |
| tsv | 149.8 | 140.6 | 145.2 | 0.94x | 0.97x |
| tsv_wasm | 85.9 | 99.3 | 103.8 | 1.16x | 1.21x |
| oxfmt | 58.1 | 58.2 | 58.3 | 1.00x | 1.00x |
| biome-wasm | 9.9 | 6.1 | — | 0.61x | — |
| malva-wasm | 19.8 | 21.8 | 18.9 | 1.11x | 0.96x |
