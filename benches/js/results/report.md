# tsv benchmark results — cross-runtime

**Generated:** 2026-07-22T09:27:01.099Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.3: d063479a @ 2026-07-22T09:08:34.891Z (tsv 0.1.0)
- `node` 24.14.1: d063479a @ 2026-07-22T09:18:34.995Z (tsv 0.1.0)
- `bun` 1.3.14: d063479a @ 2026-07-22T09:27:00.825Z (tsv 0.1.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.2 | 1.4 | 0.96x | 0.61x |
| tsv-json | 5.0 | 4.6 | 6.0 | 0.92x | 1.20x |
| tsv-json-no-locations | 7.9 | 7.3 | 8.4 | 0.93x | 1.07x |
| tsv_wasm-json | 4.1 | 4.2 | 5.6 | 1.01x | 1.36x |
| tsv_wasm-json-no-locations | 6.3 | 6.4 | 7.6 | 1.03x | 1.22x |
| tsv-internal | 50.9 | 49.2 | 52.8 | 0.97x | 1.04x |
| tsv_wasm-internal | 32.2 | 36.2 | 37.7 | 1.12x | 1.17x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.96x | 1.09x |
| tsv | 14.7 | 14.7 | 14.5 | 1.00x | 0.98x |
| tsv_wasm | 9.3 | 10.3 | 10.5 | 1.10x | 1.13x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 0.83x |
| biome-wasm | 1.4 | 1.1 | — | 0.78x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.1 | 0.91x | 0.42x |
| tsv-json | 0.6 | 0.5 | 0.7 | 0.89x | 1.30x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.3 | 0.87x | 1.10x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.97x | 1.42x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.2 | 0.98x | 1.24x |
| tsv-internal | 7.6 | 7.1 | 8.3 | 0.94x | 1.09x |
| tsv_wasm-internal | 5.0 | 5.6 | 5.9 | 1.13x | 1.19x |
| oxc-parser | 0.9 | 0.8 | 1.0 | 0.89x | 1.19x |
| oxc-parser-wasm | 0.8 | 0.7 | — | 0.95x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.78x |
| tsv | 2.0 | 2.0 | 1.9 | 0.98x | 0.97x |
| tsv_wasm | 1.3 | 1.4 | 1.5 | 1.11x | 1.14x |
| oxfmt | 1.2 | 1.2 | 1.0 | 0.99x | 0.81x |
| biome-wasm | 0.2 | 0.2 | — | 0.92x | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.10x | 1.14x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 96.8 | 109.6 | 68.2 | 1.13x | 0.70x |
| tsv-json | 67.2 | 57.7 | 74.5 | 0.86x | 1.11x |
| tsv_wasm-json | 52.0 | 55.3 | 69.7 | 1.06x | 1.34x |
| tsv-internal | 316.0 | 301.6 | 343.2 | 0.95x | 1.09x |
| tsv_wasm-internal | 178.5 | 211.1 | 218.4 | 1.18x | 1.22x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.9 | 1.8 | 2.0 | 0.92x | 1.02x |
| tsv | 154.0 | 147.6 | 153.8 | 0.96x | 1.00x |
| tsv_wasm | 88.1 | 102.7 | 104.1 | 1.17x | 1.18x |
| oxfmt | 55.8 | 52.9 | 46.7 | 0.95x | 0.84x |
| biome-wasm | 10.1 | 7.5 | — | 0.74x | — |
