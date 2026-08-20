# tsv benchmark results — cross-runtime

**Generated:** 2026-08-20T12:30:46.813Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.4: 8a87d997 @ 2026-08-20T02:38:59.613Z (tsv 0.2.0)
- `node` 24.14.1: 8a87d997 @ 2026-08-20T02:49:33.411Z (tsv 0.2.0)
- `bun` 1.3.14: 8a87d997 @ 2026-08-20T02:58:53.219Z (tsv 0.2.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — oxc-parser-wasm, biome-wasm. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Within noise:** 6 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/svelte/compiler` node (0.8% vs 4.4% noise); `format/svelte/prettier` node (2.3% vs 3.7% noise); `format/svelte/tsv` node (0.8% vs 1.5% noise); `parse/css/svelte/compiler` node (3.5% vs 5.5% noise); `parse/css/postcss` node (1.2% vs 4.3% noise); `format/css/oxfmt` node (0.1% vs 4.9% noise). Read those cells as "no difference", and see each per-runtime report's §Unstable Rows for the noisy row itself.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.1 | 2.0 | 1.4 | 0.99x | 0.70x |
| tsv-json | 5.0 | 4.6 | 6.1 | 0.92x | 1.22x |
| tsv_wasm-json | 4.3 | 4.3 | 5.8 | 1.01x | 1.36x |
| tsv-json-no-locations | 7.8 | 7.2 | 8.5 | 0.93x | 1.09x |
| tsv_wasm-json-no-locations | 6.4 | 6.5 | 7.9 | 1.02x | 1.24x |
| tsv-internal | 51.4 | 49.4 | 52.8 | 0.96x | 1.03x |
| tsv_wasm-internal | 32.6 | 36.0 | 38.3 | 1.10x | 1.17x |
| rsvelte-parse | 2.8 | 2.7 | 3.1 | 0.95x | 1.08x |
| rsvelte-parse-skip-expr-loc | 4.8 | 4.6 | 4.9 | 0.95x | 1.03x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.98x | 1.12x |
| tsv | 13.4 | 13.3 | 12.9 | 0.99x | 0.97x |
| tsv_wasm | 8.4 | 9.3 | 9.3 | 1.11x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 0.83x |
| biome-wasm | 1.3 | 1.1 | — | 0.81x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.96x | 0.55x |
| tsv-json | 0.5 | 0.5 | 0.7 | 0.90x | 1.37x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.98x | 1.52x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.2 | 0.88x | 1.12x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.2 | 0.99x | 1.27x |
| tsv-internal | 7.7 | 7.0 | 8.1 | 0.92x | 1.06x |
| tsv_wasm-internal | 5.1 | 5.5 | 6.0 | 1.09x | 1.19x |
| oxc-parser | 0.8 | 0.7 | 1.0 | 0.90x | 1.21x |
| oxc-parser-wasm | 0.7 | 0.7 | — | 0.96x | — |
| yuku-parser | 2.2 | 2.4 | 2.9 | 1.11x | 1.31x |
| yuku-parser-wasm | 2.5 | 2.9 | 3.7 | 1.14x | 1.47x |
| swc | 0.6 | 0.6 | 0.7 | 0.92x | 1.13x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.91x | 0.82x |
| tsv | 1.8 | 1.7 | 1.7 | 0.97x | 0.96x |
| tsv_wasm | 1.1 | 1.3 | 1.3 | 1.10x | 1.13x |
| oxfmt | 1.2 | 1.1 | 1.0 | 0.97x | 0.82x |
| biome-wasm | 0.2 | 0.2 | — | 0.95x | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 110.4 | 106.5 | 67.5 | 0.97x | 0.61x |
| tsv-json | 65.2 | 56.3 | 69.8 | 0.86x | 1.07x |
| tsv_wasm-json | 51.4 | 54.6 | 70.3 | 1.06x | 1.37x |
| tsv-internal | 325.6 | 297.8 | 337.4 | 0.91x | 1.04x |
| tsv_wasm-internal | 184.2 | 216.9 | 233.0 | 1.18x | 1.26x |
| postcss | 99.1 | 100.3 | 87.2 | 1.01x | 0.88x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.9 | 1.8 | 2.1 | 0.96x | 1.12x |
| tsv | 154.1 | 143.5 | 148.0 | 0.93x | 0.96x |
| tsv_wasm | 85.9 | 101.9 | 102.8 | 1.19x | 1.20x |
| oxfmt | 58.3 | 58.4 | 48.9 | 1.00x | 0.84x |
| biome-wasm | 10.0 | 5.2 | — | 0.52x | — |
| malva-wasm | 20.2 | 21.9 | 17.8 | 1.08x | 0.88x |
