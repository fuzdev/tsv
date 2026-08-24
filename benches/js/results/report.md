# tsv benchmark results — cross-runtime

**Generated:** 2026-08-24T13:01:27.954Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.5: 5950c4ad @ 2026-08-24T01:05:57.258Z (tsv 0.2.0)
- `node` 24.14.1: 5950c4ad @ 2026-08-24T01:16:33.590Z (tsv 0.2.0)
- `bun` 1.4.0: 5950c4ad @ 2026-08-24T01:25:14.821Z (tsv 0.2.0)
- `conformance` (node, coverage-only): 5950c4ad @ 2026-08-24T13:00:43.410Z (tsv 0.2.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — biome-wasm. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Within noise:** 9 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/svelte/compiler` node (2.0% vs 3.7% noise, n=10/11); `parse/svelte/tsv_wasm-json` node (0.4% vs 0.7% noise, n=18/19); `parse/svelte/tsv-internal` node (1.0% vs 1.4% noise, n=173/202); `format/svelte/tsv` node (0.2% vs 1.6% noise, n=59/49); `format/svelte/tsv` bun (1.0% vs 2.0% noise, n=59/65); `parse/css/svelte/compiler` node (3.7% vs 6.1% noise, n=547/506); `parse/css/postcss` node (1.0% vs 4.4% noise, n=488/480); `format/css/oxfmt` node (2.5% vs 4.4% noise, n=295/282); `format/css/oxfmt` bun (2.2% vs 4.6% noise, n=295/283). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.1 | 2.0 | 1.6 | 0.98x | 0.75x |
| tsv-json | 5.2 | 4.7 | 6.9 | 0.90x | 1.34x |
| tsv_wasm-json | 4.4 | 4.4 | 6.7 | 1.00x | 1.52x |
| tsv-json-no-locations | 8.2 | 7.5 | 9.6 | 0.92x | 1.18x |
| tsv_wasm-json-no-locations | 6.5 | 6.7 | 9.0 | 1.03x | 1.38x |
| tsv-internal | 49.7 | 49.2 | 53.6 | 0.99x | 1.08x |
| tsv_wasm-internal | 31.9 | 35.0 | 36.7 | 1.10x | 1.15x |
| rsvelte-parse | 2.9 | 2.7 | 3.3 | 0.94x | 1.14x |
| rsvelte-parse-skip-expr-loc | 4.8 | 4.6 | 5.1 | 0.95x | 1.06x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.98x | 1.27x |
| tsv | 13.1 | 13.1 | 13.0 | 1.00x | 0.99x |
| tsv_wasm | 8.5 | 9.2 | 9.4 | 1.09x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.3 | 0.98x | 1.19x |
| biome-wasm | 1.3 | 1.1 | — | 0.81x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.94x | 0.64x |
| tsv-json | 0.5 | 0.5 | 0.8 | 0.88x | 1.54x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.96x | 1.77x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.4 | 0.88x | 1.31x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.4 | 0.98x | 1.50x |
| tsv-internal | 7.5 | 7.0 | 8.1 | 0.93x | 1.08x |
| tsv_wasm-internal | 5.0 | 5.4 | 5.8 | 1.08x | 1.16x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.89x | 1.40x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.8 | 0.96x | 1.11x |
| yuku-parser | 2.2 | 2.4 | 3.0 | 1.10x | 1.37x |
| yuku-parser-wasm | 2.4 | 2.9 | 3.6 | 1.18x | 1.47x |
| swc | 0.6 | 0.6 | 0.7 | 0.93x | 1.21x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.91x | 0.95x |
| tsv | 1.8 | 1.7 | 1.7 | 0.97x | 0.97x |
| tsv_wasm | 1.2 | 1.3 | 1.3 | 1.09x | 1.13x |
| oxfmt | 1.2 | 1.1 | 1.2 | 0.99x | 1.00x |
| biome-wasm | 0.2 | 0.2 | — | 0.94x | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 109.9 | 105.8 | 65.0 | 0.96x | 0.59x |
| tsv-json | 63.6 | 55.6 | 72.3 | 0.87x | 1.14x |
| tsv_wasm-json | 51.3 | 53.3 | 74.7 | 1.04x | 1.45x |
| tsv-internal | 307.6 | 287.3 | 324.8 | 0.93x | 1.06x |
| tsv_wasm-internal | 175.1 | 202.7 | 224.1 | 1.16x | 1.28x |
| postcss | 99.1 | 100.1 | 84.8 | 1.01x | 0.86x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.9 | 1.8 | 2.5 | 0.94x | 1.30x |
| tsv | 146.2 | 132.7 | 140.8 | 0.91x | 0.96x |
| tsv_wasm | 84.0 | 96.2 | 101.0 | 1.15x | 1.20x |
| oxfmt | 59.5 | 58.0 | 58.1 | 0.98x | 0.98x |
| biome-wasm | 9.9 | 5.3 | — | 0.53x | — |
| malva-wasm | 20.1 | 21.9 | 18.1 | 1.08x | 0.90x |
