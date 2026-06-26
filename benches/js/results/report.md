# tsv benchmark results — cross-runtime

**Generated:** 2026-06-26T16:14:57.526Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno).

## parse/svelte

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.2 | 2.2 | 1.4 | 0.97x | 0.61x |
| tsv-json | 1.7 | 1.8 | 2.1 | 1.05x | 1.22x |
| tsv_wasm-json | 1.7 | 1.8 | 2.1 | 1.11x | 1.26x |
| tsv-internal | 30.0 | 35.9 | 38.4 | 1.20x | 1.28x |
| tsv_wasm-internal | 22.3 | 25.9 | 27.4 | 1.16x | 1.23x |

## format/svelte

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.96x | 0.92x |
| tsv | 2.9 | 4.1 | 3.9 | 1.43x | 1.34x |
| tsv_wasm | 2.6 | 2.8 | 2.8 | 1.10x | 1.10x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.98x | 0.77x |
| biome-wasm | 1.3 | 1.4 | — | 1.14x | — |

## parse/typescript

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.96x | 0.47x |
| tsv-json | 0.3 | 0.4 | 0.5 | 1.20x | 1.65x |
| tsv_wasm-json | 0.3 | 0.3 | 0.5 | 1.01x | 1.37x |
| tsv-internal | 4.4 | 4.8 | 5.3 | 1.10x | 1.20x |
| tsv_wasm-internal | 3.1 | 3.8 | 3.9 | 1.22x | 1.27x |
| oxc-parser | 0.8 | 0.8 | 1.0 | 0.92x | 1.23x |
| oxc-parser-wasm | 0.8 | 0.7 | — | 0.96x | — |

## format/typescript

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.93x | 0.80x |
| tsv | 1.1 | 1.2 | 1.2 | 1.13x | 1.16x |
| tsv_wasm | 0.8 | 1.0 | 1.0 | 1.16x | 1.16x |
| oxfmt | 0.9 | 1.0 | 0.7 | 1.04x | 0.79x |
| biome-wasm | 0.2 | 0.2 | — | 1.16x | — |

## parse/css

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 189.4 | 176.2 | 136.2 | 0.93x | 0.72x |
| tsv-json | 66.5 | 77.1 | 87.9 | 1.16x | 1.32x |
| tsv_wasm-json | 60.9 | 66.0 | 71.2 | 1.08x | 1.17x |
| tsv-internal | 179.6 | 207.3 | 220.6 | 1.15x | 1.23x |
| tsv_wasm-internal | 130.7 | 150.6 | 151.0 | 1.15x | 1.16x |

## format/css

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 4.0 | 3.8 | 3.9 | 0.95x | 0.98x |
| tsv | 79.2 | 101.1 | 105.6 | 1.28x | 1.33x |
| tsv_wasm | 66.8 | 77.4 | 78.0 | 1.16x | 1.17x |
| oxfmt | 3.6 | 3.5 | 2.8 | 0.96x | 0.79x |
| biome-wasm | 14.8 | 18.2 | — | 1.23x | — |
