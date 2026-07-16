# tsv benchmark results — cross-runtime

**Generated:** 2026-07-16T11:51:31.514Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.8.3: 135b7b93 @ 2026-07-16T11:34:23.572Z (tsv 0.1.0)
- `node` 24.14.1: 135b7b93 @ 2026-07-16T11:43:29.034Z (tsv 0.1.0)
- `bun` 1.3.14: 135b7b93 @ 2026-07-16T11:51:31.124Z (tsv 0.1.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.2 | 1.4 | 0.95x | 0.58x |
| tsv-json | 5.1 | 4.6 | 6.0 | 0.91x | 1.18x |
| tsv-json-no-locations | 7.9 | 7.3 | 8.5 | 0.93x | 1.08x |
| tsv_wasm-json | 4.2 | 4.1 | 5.6 | 0.99x | 1.34x |
| tsv_wasm-json-no-locations | 6.3 | 6.4 | 7.7 | 1.01x | 1.22x |
| tsv-internal | 47.0 | 47.9 | 51.1 | 1.02x | 1.09x |
| tsv_wasm-internal | 30.5 | 26.8 | 15.9 | 0.88x | 0.52x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.95x | 1.06x |
| tsv | 14.1 | 14.4 | 14.1 | 1.02x | 1.00x |
| tsv_wasm | 9.1 | 7.9 | 7.2 | 0.88x | 0.79x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.96x | 0.79x |
| biome-wasm | 1.4 | 1.1 | — | 0.76x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.2 | 0.92x | 0.43x |
| tsv-json | 0.6 | 0.5 | 0.7 | 0.88x | 1.34x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.3 | 0.87x | 1.12x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.96x | 1.44x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.2 | 0.97x | 1.24x |
| tsv-internal | 7.6 | 7.2 | 8.3 | 0.95x | 1.09x |
| tsv_wasm-internal | 4.8 | 5.4 | 5.7 | 1.12x | 1.18x |
| oxc-parser | 0.9 | 0.8 | 1.0 | 0.87x | 1.22x |
| oxc-parser-wasm | 0.8 | 0.7 | — | 0.96x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.76x |
| tsv | 2.0 | 2.0 | 1.9 | 0.98x | 0.97x |
| tsv_wasm | 1.3 | 1.4 | 1.4 | 1.10x | 1.13x |
| oxfmt | 1.2 | 1.2 | 1.0 | 0.98x | 0.83x |
| biome-wasm | 0.2 | 0.2 | — | 0.88x | — |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 99.3 | 110.6 | 68.2 | 1.11x | 0.69x |
| tsv-json | 66.6 | 57.5 | 73.6 | 0.86x | 1.11x |
| tsv_wasm-json | 52.3 | 54.7 | 68.5 | 1.04x | 1.31x |
| tsv-internal | 304.8 | 291.9 | 325.2 | 0.96x | 1.07x |
| tsv_wasm-internal | 184.8 | 203.9 | 217.8 | 1.10x | 1.18x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.9 | 1.8 | 1.9 | 0.94x | 0.99x |
| tsv | 148.6 | 142.2 | 147.5 | 0.96x | 0.99x |
| tsv_wasm | 88.4 | 98.4 | 101.4 | 1.11x | 1.15x |
| oxfmt | 56.4 | 55.3 | 47.4 | 0.98x | 0.84x |
| biome-wasm | 10.3 | 7.9 | — | 0.77x | — |
