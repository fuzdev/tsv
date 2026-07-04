# tsv benchmark results — cross-runtime

**Generated:** 2026-07-04T17:17:03.572Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno).

## parse/svelte

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.2 | 1.2 | 0.94x | 0.50x |
| tsv-json | 4.8 | 4.5 | 4.6 | 0.94x | 0.95x |
| tsv_wasm-json | 4.1 | 4.0 | 4.9 | 0.99x | 1.20x |
| tsv-internal | 38.0 | 43.5 | 29.3 | 1.15x | 0.77x |
| tsv_wasm-internal | 28.1 | 23.9 | 12.3 | 0.85x | 0.44x |

## format/svelte

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.94x | 0.90x |
| tsv | 11.5 | 12.9 | 11.9 | 1.12x | 1.03x |
| tsv_wasm | 8.0 | 7.2 | 6.5 | 0.90x | 0.81x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.99x | 0.76x |
| biome-wasm | 1.3 | 1.4 | — | 1.13x | — |

## parse/typescript

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.93x | 0.45x |
| tsv-json | 0.5 | 0.5 | 0.7 | 0.93x | 1.43x |
| tsv_wasm-json | 0.5 | 0.4 | 0.7 | 0.97x | 1.46x |
| tsv-internal | 5.8 | 5.7 | 7.1 | 0.99x | 1.22x |
| tsv_wasm-internal | 4.2 | 4.8 | 4.8 | 1.12x | 1.14x |
| oxc-parser | 0.8 | 0.7 | 1.0 | 0.88x | 1.21x |
| oxc-parser-wasm | 0.7 | 0.7 | — | 0.97x | — |

## format/typescript

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.93x | 0.82x |
| tsv | 1.6 | 1.7 | 1.7 | 1.06x | 1.06x |
| tsv_wasm | 1.1 | 1.3 | 1.2 | 1.12x | 1.10x |
| oxfmt | 0.9 | 1.0 | 0.7 | 1.03x | 0.76x |
| biome-wasm | 0.2 | 0.2 | — | 1.15x | — |

## parse/css

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 197.1 | 170.3 | 134.8 | 0.86x | 0.68x |
| tsv-json | 104.4 | 106.1 | 125.4 | 1.02x | 1.20x |
| tsv_wasm-json | 83.9 | 88.0 | 103.5 | 1.05x | 1.23x |
| tsv-internal | 254.7 | 285.7 | 298.7 | 1.12x | 1.17x |
| tsv_wasm-internal | 178.9 | 189.6 | 203.9 | 1.06x | 1.14x |

## format/css

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 3.2 | 2.2 | 3.0 | 0.70x | 0.95x |
| tsv | 143.5 | 106.2 | 160.6 | 0.74x | 1.12x |
| tsv_wasm | 96.8 | 108.5 | 114.4 | 1.12x | 1.18x |
| oxfmt | 3.7 | 3.2 | 2.7 | 0.86x | 0.74x |
| biome-wasm | 15.5 | 16.8 | — | 1.08x | — |
