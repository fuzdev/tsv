# tsv benchmark results — cross-runtime

**Generated:** 2026-07-16T00:23:32.612Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.8.3: eca5466f @ 2026-07-16T00:07:00.449Z (tsv 0.1.0)
- `node` 24.14.1: eca5466f @ 2026-07-16T00:15:41.741Z (tsv 0.1.0)
- `bun` 1.3.14: eca5466f @ 2026-07-16T00:23:32.242Z (tsv 0.1.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.2 | 1.4 | 0.98x | 0.59x |
| tsv-json | 5.0 | 4.6 | 6.0 | 0.93x | 1.21x |
| tsv-json-no-locations | 7.8 | 7.3 | 8.5 | 0.95x | 1.09x |
| tsv_wasm-json | 4.1 | 4.2 | 5.6 | 1.02x | 1.36x |
| tsv_wasm-json-no-locations | 6.2 | 6.4 | 7.6 | 1.03x | 1.22x |
| tsv-internal | 46.2 | 47.4 | 50.6 | 1.03x | 1.10x |
| tsv_wasm-internal | 30.2 | 27.1 | 16.1 | 0.90x | 0.53x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.3 | 0.96x | 1.15x |
| tsv | 14.1 | 14.6 | 14.0 | 1.03x | 0.99x |
| tsv_wasm | 9.0 | 8.0 | 7.2 | 0.88x | 0.80x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.96x | 0.83x |
| biome-wasm | 1.4 | 1.1 | — | 0.76x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.4 | 0.3 | 0.1 | 0.90x | 0.42x |
| tsv-json | 0.5 | 0.5 | 0.7 | 0.91x | 1.35x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.3 | 0.88x | 1.13x |
| tsv_wasm-json | 0.5 | 0.5 | 0.7 | 0.98x | 1.44x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.2 | 0.99x | 1.26x |
| tsv-internal | 7.6 | 7.2 | 8.3 | 0.95x | 1.10x |
| tsv_wasm-internal | 4.8 | 5.5 | 5.8 | 1.14x | 1.20x |
| oxc-parser | 0.9 | 0.8 | 1.0 | 0.89x | 1.21x |
| oxc-parser-wasm | 0.8 | 0.7 | — | 0.98x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.89x | 0.81x |
| tsv | 2.0 | 2.0 | 1.9 | 0.98x | 0.97x |
| tsv_wasm | 1.3 | 1.4 | 1.4 | 1.10x | 1.14x |
| oxfmt | 1.2 | 1.2 | 1.0 | 0.99x | 0.82x |
| biome-wasm | 0.2 | 0.2 | — | 0.91x | — |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 98.9 | 107.0 | 68.9 | 1.08x | 0.70x |
| tsv-json | 66.3 | 57.6 | 73.5 | 0.87x | 1.11x |
| tsv_wasm-json | 51.9 | 55.2 | 68.5 | 1.06x | 1.32x |
| tsv-internal | 302.2 | 289.9 | 327.6 | 0.96x | 1.08x |
| tsv_wasm-internal | 181.9 | 205.0 | 218.9 | 1.13x | 1.20x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 2.0 | 1.8 | 2.2 | 0.93x | 1.10x |
| tsv | 148.6 | 143.4 | 147.5 | 0.97x | 0.99x |
| tsv_wasm | 87.9 | 99.9 | 101.7 | 1.14x | 1.16x |
| oxfmt | 55.4 | 54.2 | 45.7 | 0.98x | 0.82x |
| biome-wasm | 10.3 | 7.9 | — | 0.77x | — |
