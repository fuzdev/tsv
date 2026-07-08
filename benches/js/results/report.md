# tsv benchmark results — cross-runtime

**Generated:** 2026-07-08T02:14:34.907Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.8.3: 99ac4c69 @ 2026-07-08T01:58:22.087Z (tsv 0.1.0)
- `node` 24.14.1: 99ac4c69 @ 2026-07-08T02:06:48.550Z (tsv 0.1.0)
- `bun` 1.3.14: 99ac4c69 @ 2026-07-08T02:14:34.550Z (tsv 0.1.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.4 | 2.3 | 1.4 | 0.97x | 0.59x |
| tsv-json | 5.1 | 4.8 | 6.1 | 0.93x | 1.20x |
| tsv-json-no-locations | 7.9 | 7.5 | 8.6 | 0.95x | 1.08x |
| tsv_wasm-json | 4.2 | 4.3 | 5.6 | 1.02x | 1.33x |
| tsv_wasm-json-no-locations | 6.3 | 6.6 | 7.8 | 1.04x | 1.23x |
| tsv-internal | 46.6 | 46.7 | 49.3 | 1.00x | 1.06x |
| tsv_wasm-internal | 30.3 | 23.0 | 15.6 | 0.76x | 0.52x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.96x | 1.07x |
| tsv | 12.9 | 12.9 | 12.7 | 1.01x | 0.99x |
| tsv_wasm | 8.3 | 8.2 | 9.2 | 0.99x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 0.78x |
| biome-wasm | 1.5 | 1.1 | — | 0.76x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.2 | 0.93x | 0.45x |
| tsv-json | 0.6 | 0.5 | 0.8 | 0.90x | 1.34x |
| tsv-json-no-locations | 1.2 | 1.0 | 1.3 | 0.90x | 1.13x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.98x | 1.43x |
| tsv_wasm-json-no-locations | 1.0 | 1.0 | 1.2 | 1.00x | 1.26x |
| tsv-internal | 7.4 | 7.3 | 8.1 | 0.99x | 1.10x |
| tsv_wasm-internal | 4.9 | 5.4 | 5.5 | 1.11x | 1.14x |
| oxc-parser | 0.9 | 0.8 | 1.1 | 0.94x | 1.26x |
| oxc-parser-wasm | 0.8 | 0.8 | — | 0.99x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.92x | 0.81x |
| tsv | 1.9 | 1.9 | 1.9 | 0.99x | 0.98x |
| tsv_wasm | 1.2 | 1.3 | 1.4 | 1.03x | 1.12x |
| oxfmt | 1.1 | 1.1 | 0.9 | 1.02x | 0.86x |
| biome-wasm | 0.3 | 0.2 | — | 0.92x | — |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 109.4 | 122.9 | 76.9 | 1.12x | 0.70x |
| tsv-json | 59.6 | 52.4 | 65.2 | 0.88x | 1.10x |
| tsv_wasm-json | 45.4 | 43.0 | 58.7 | 0.95x | 1.29x |
| tsv-internal | 195.2 | 187.8 | 201.8 | 0.96x | 1.03x |
| tsv_wasm-internal | 118.2 | 95.7 | 136.9 | 0.81x | 1.16x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 2.1 | 1.9 | 2.0 | 0.91x | 1.00x |
| tsv | 109.0 | 105.0 | 107.5 | 0.96x | 0.99x |
| tsv_wasm | 64.7 | 62.4 | 76.2 | 0.97x | 1.18x |
| oxfmt | 11.7 | 11.3 | 10.6 | 0.97x | 0.90x |
| biome-wasm | 11.1 | 6.3 | — | 0.57x | — |
