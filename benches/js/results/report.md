# tsv benchmark results — cross-runtime

**Generated:** 2026-07-07T20:20:24.091Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.8.3: 3ee86763 @ 2026-07-07T20:02:16.112Z (tsv 0.1.0)
- `node` 24.14.1: 3ee86763 @ 2026-07-07T20:11:53.386Z (tsv 0.1.0)
- `bun` 1.3.14: 3ee86763 @ 2026-07-07T20:20:23.755Z (tsv 0.1.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.4 | 2.3 | 1.4 | 0.95x | 0.60x |
| tsv-json | 5.2 | 4.7 | 6.1 | 0.90x | 1.19x |
| tsv-json-no-locations | 8.1 | 7.1 | 8.6 | 0.88x | 1.06x |
| tsv_wasm-json | 4.3 | 4.1 | 5.7 | 0.97x | 1.33x |
| tsv_wasm-json-no-locations | 6.4 | 6.4 | 7.7 | 1.00x | 1.20x |
| tsv-internal | 46.6 | 45.5 | 49.4 | 0.98x | 1.06x |
| tsv_wasm-internal | 30.4 | 23.3 | 15.7 | 0.77x | 0.52x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.98x | 1.15x |
| tsv | 12.8 | 13.1 | 12.8 | 1.03x | 1.00x |
| tsv_wasm | 8.3 | 8.5 | 9.3 | 1.03x | 1.13x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 0.84x |
| biome-wasm | 1.5 | 1.1 | — | 0.77x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.2 | 0.91x | 0.44x |
| tsv-json | 0.6 | 0.5 | 0.8 | 0.90x | 1.34x |
| tsv-json-no-locations | 1.2 | 1.1 | 1.3 | 0.89x | 1.13x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.99x | 1.45x |
| tsv_wasm-json-no-locations | 1.0 | 1.0 | 1.2 | 0.99x | 1.25x |
| tsv-internal | 7.4 | 7.3 | 8.3 | 0.98x | 1.12x |
| tsv_wasm-internal | 4.9 | 5.5 | 5.6 | 1.13x | 1.14x |
| oxc-parser | 0.9 | 0.8 | 1.1 | 0.90x | 1.21x |
| oxc-parser-wasm | 0.8 | 0.8 | — | 0.97x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.91x | 0.82x |
| tsv | 1.9 | 1.9 | 1.9 | 0.99x | 0.99x |
| tsv_wasm | 1.2 | 1.4 | 1.4 | 1.10x | 1.12x |
| oxfmt | 1.1 | 1.1 | 1.0 | 0.99x | 0.83x |
| biome-wasm | 0.3 | 0.2 | — | 0.92x | — |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 103.2 | 115.6 | 73.7 | 1.12x | 0.71x |
| tsv-json | 58.4 | 51.8 | 63.6 | 0.89x | 1.09x |
| tsv_wasm-json | 45.1 | 47.3 | 56.7 | 1.05x | 1.26x |
| tsv-internal | 180.6 | 175.7 | 189.0 | 0.97x | 1.05x |
| tsv_wasm-internal | 114.5 | 121.4 | 129.3 | 1.06x | 1.13x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 2.0 | 1.9 | 2.2 | 0.95x | 1.11x |
| tsv | 103.1 | 100.3 | 103.6 | 0.97x | 1.01x |
| tsv_wasm | 64.0 | 70.2 | 73.1 | 1.10x | 1.14x |
| oxfmt | 11.4 | 11.3 | 10.3 | 0.99x | 0.90x |
| biome-wasm | 11.0 | 6.0 | — | 0.54x | — |
