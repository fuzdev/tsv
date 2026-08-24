# tsv benchmark results — cross-runtime

**Generated:** 2026-08-24T22:26:24.624Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.5: 3d0eea49 @ 2026-08-24T22:05:43.811Z (tsv 0.2.0)
- `node` 24.14.1: 3d0eea49 @ 2026-08-24T22:16:26.009Z (tsv 0.2.0)
- `bun` 1.4.0: 3d0eea49 @ 2026-08-24T22:25:06.303Z (tsv 0.2.0)
- `conformance` (node, coverage-only): 3d0eea49 @ 2026-08-24T22:26:24.331Z (tsv 0.2.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — biome-wasm. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Within noise:** 10 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/svelte/compiler` node (1.8% vs 4.0% noise, n=11/11); `parse/svelte/tsv_wasm-json` node (0.9% vs 1.3% noise, n=21/19); `parse/svelte/tsv_wasm-json-no-locations` node (1.0% vs 1.5% noise, n=29/30); `parse/svelte/tsv-internal` node (0.7% vs 1.8% noise, n=189/200); `format/svelte/tsv` node (1.9% vs 2.6% noise, n=65/61); `format/svelte/tsv` bun (1.2% vs 2.7% noise, n=65/66); `parse/css/svelte/compiler` node (2.8% vs 5.7% noise, n=545/501); `parse/css/postcss` node (1.7% vs 4.7% noise, n=484/473); `format/css/oxfmt` node (3.5% vs 4.3% noise, n=294/284); `format/css/oxfmt` bun (1.0% vs 4.2% noise, n=294/288). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.1 | 2.0 | 1.6 | 0.98x | 0.77x |
| tsv-json | 5.2 | 4.6 | 6.9 | 0.89x | 1.33x |
| tsv_wasm-json | 4.4 | 4.3 | 6.7 | 0.99x | 1.53x |
| tsv-json-no-locations | 8.1 | 7.5 | 9.8 | 0.93x | 1.22x |
| tsv_wasm-json-no-locations | 6.6 | 6.7 | 8.8 | 1.01x | 1.34x |
| tsv-internal | 49.5 | 49.2 | 52.9 | 0.99x | 1.07x |
| tsv_wasm-internal | 31.9 | 34.7 | 35.9 | 1.09x | 1.13x |
| rsvelte-parse | 2.9 | 2.7 | 3.2 | 0.93x | 1.13x |
| rsvelte-parse-skip-expr-loc | 4.8 | 4.5 | 5.1 | 0.94x | 1.07x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.96x | 1.26x |
| tsv | 12.9 | 13.2 | 13.1 | 1.02x | 1.01x |
| tsv_wasm | 8.3 | 9.3 | 9.4 | 1.11x | 1.12x |
| oxfmt | 0.2 | 0.2 | 0.3 | 0.97x | 1.15x |
| biome-wasm | 1.3 | 1.0 | — | 0.79x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.95x | 0.63x |
| tsv-json | 0.5 | 0.5 | 0.8 | 0.87x | 1.55x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.95x | 1.79x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.5 | 0.88x | 1.33x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.4 | 0.97x | 1.49x |
| tsv-internal | 7.4 | 7.0 | 8.1 | 0.94x | 1.10x |
| tsv_wasm-internal | 5.1 | 5.2 | 5.7 | 1.03x | 1.13x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.89x | 1.38x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.8 | 0.96x | 1.12x |
| yuku-parser | 2.2 | 2.4 | 2.9 | 1.11x | 1.32x |
| yuku-parser-wasm | 2.5 | 2.9 | 3.8 | 1.17x | 1.54x |
| swc | 0.6 | 0.6 | 0.7 | 0.93x | 1.22x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.99x |
| tsv | 1.8 | 1.7 | 1.7 | 0.98x | 0.99x |
| tsv_wasm | 1.2 | 1.3 | 1.3 | 1.09x | 1.12x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.98x | 1.00x |
| biome-wasm | 0.2 | 0.2 | — | 0.94x | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.11x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 109.5 | 106.4 | 65.2 | 0.97x | 0.60x |
| tsv-json | 64.4 | 55.4 | 72.3 | 0.86x | 1.12x |
| tsv_wasm-json | 51.2 | 53.1 | 73.7 | 1.04x | 1.44x |
| tsv-internal | 318.0 | 300.0 | 340.0 | 0.94x | 1.07x |
| tsv_wasm-internal | 173.8 | 205.2 | 223.5 | 1.18x | 1.29x |
| postcss | 98.6 | 100.3 | 84.5 | 1.02x | 0.86x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 2.0 | 1.7 | 2.5 | 0.89x | 1.30x |
| tsv | 151.1 | 141.4 | 146.8 | 0.94x | 0.97x |
| tsv_wasm | 84.7 | 98.1 | 102.8 | 1.16x | 1.21x |
| oxfmt | 59.2 | 57.1 | 58.6 | 0.97x | 0.99x |
| biome-wasm | 9.8 | 6.1 | — | 0.62x | — |
| malva-wasm | 20.1 | 21.9 | 18.4 | 1.09x | 0.91x |
