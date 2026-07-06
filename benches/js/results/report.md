# tsv benchmark results — cross-runtime

**Generated:** 2026-07-06T22:45:31.985Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno`: a99ef299 @ 2026-07-06T22:27:28.070Z (tsv 0.1.0)
- `node`: a99ef299 @ 2026-07-06T22:36:55.632Z (tsv 0.1.0)
- `bun`: a99ef299 @ 2026-07-06T22:45:31.629Z (tsv 0.1.0)

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.2 | 1.4 | 0.95x | 0.60x |
| tsv-json | 5.1 | 4.7 | 6.1 | 0.92x | 1.19x |
| tsv-json-no-locations | 8.0 | 7.4 | 8.5 | 0.93x | 1.07x |
| tsv_wasm-json | 4.2 | 4.2 | 5.6 | 1.01x | 1.34x |
| tsv_wasm-json-no-locations | 6.3 | 6.5 | 7.7 | 1.02x | 1.21x |
| tsv-internal | 46.0 | 46.2 | 49.4 | 1.00x | 1.08x |
| tsv_wasm-internal | 29.8 | 22.8 | 15.6 | 0.77x | 0.52x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.94x | 1.11x |
| tsv | 12.7 | 12.9 | 12.6 | 1.02x | 0.99x |
| tsv_wasm | 8.2 | 8.4 | 9.2 | 1.03x | 1.12x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 0.85x |
| biome-wasm | 1.5 | 1.1 | — | 0.78x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.2 | 0.90x | 0.45x |
| tsv-json | 0.6 | 0.5 | 0.8 | 0.90x | 1.33x |
| tsv-json-no-locations | 1.2 | 1.0 | 1.3 | 0.88x | 1.12x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.99x | 1.43x |
| tsv_wasm-json-no-locations | 1.0 | 1.0 | 1.2 | 0.98x | 1.24x |
| tsv-internal | 7.4 | 7.2 | 8.2 | 0.97x | 1.11x |
| tsv_wasm-internal | 4.9 | 5.5 | 5.6 | 1.11x | 1.14x |
| oxc-parser | 0.9 | 0.8 | 1.1 | 0.89x | 1.21x |
| oxc-parser-wasm | 0.8 | 0.8 | — | 0.98x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.83x |
| tsv | 1.9 | 1.9 | 1.9 | 1.00x | 0.99x |
| tsv_wasm | 1.2 | 1.4 | 1.4 | 1.11x | 1.12x |
| oxfmt | 1.1 | 1.1 | 1.0 | 0.97x | 0.84x |
| biome-wasm | 0.3 | 0.2 | — | 0.92x | — |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 109.1 | 122.1 | 77.9 | 1.12x | 0.71x |
| tsv-json | 58.7 | 51.5 | 64.0 | 0.88x | 1.09x |
| tsv_wasm-json | 45.0 | 47.3 | 57.6 | 1.05x | 1.28x |
| tsv-internal | 192.1 | 187.2 | 200.7 | 0.97x | 1.04x |
| tsv_wasm-internal | 117.4 | 127.0 | 134.1 | 1.08x | 1.14x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 2.1 | 1.9 | 2.1 | 0.91x | 1.03x |
| tsv | 107.2 | 103.7 | 106.1 | 0.97x | 0.99x |
| tsv_wasm | 64.5 | 71.0 | 75.0 | 1.10x | 1.16x |
| oxfmt | 10.7 | 11.0 | 10.3 | 1.02x | 0.96x |
| biome-wasm | 10.9 | 6.3 | — | 0.58x | — |
