# tsv benchmark results — cross-runtime

**Generated:** 2026-07-03T21:13:05.215Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno).

## parse/svelte

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.3 | 2.2 | 1.4 | 0.98x | 0.61x |
| tsv-json | 2.9 | 2.9 | 3.5 | 1.01x | 1.19x |
| tsv_wasm-json | 2.4 | 2.6 | 3.1 | 1.08x | 1.28x |
| tsv-internal | 35.9 | 44.0 | 46.4 | 1.22x | 1.29x |
| tsv_wasm-internal | 26.0 | 19.8 | 9.3 | 0.76x | 0.36x |

## format/svelte

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.99x | 1.09x |
| tsv | 9.9 | 11.8 | 11.4 | 1.19x | 1.15x |
| tsv_wasm | 7.4 | 7.2 | 8.4 | 0.97x | 1.13x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.97x | 0.77x |
| biome-wasm | 1.3 | 1.5 | — | 1.14x | — |

## parse/typescript

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.94x | 0.47x |
| tsv-json | 0.5 | 0.5 | 0.7 | 0.94x | 1.44x |
| tsv_wasm-json | 0.4 | 0.4 | 0.7 | 1.00x | 1.50x |
| tsv-internal | 5.6 | 6.3 | 6.9 | 1.13x | 1.24x |
| tsv_wasm-internal | 3.9 | 4.5 | 4.5 | 1.16x | 1.17x |
| oxc-parser | 0.8 | 0.8 | 1.1 | 0.93x | 1.25x |
| oxc-parser-wasm | 0.8 | 0.8 | — | 0.97x | — |

## format/typescript

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.88x | 0.84x |
| tsv | 1.5 | 1.7 | 1.6 | 1.09x | 1.07x |
| tsv_wasm | 1.1 | 1.2 | 1.2 | 1.13x | 1.13x |
| oxfmt | 1.0 | 1.0 | 0.7 | 1.02x | 0.76x |
| biome-wasm | 0.2 | 0.2 | — | 1.14x | — |

## parse/css

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 196.1 | 180.7 | 139.2 | 0.92x | 0.71x |
| tsv-json | 104.9 | 109.0 | 130.7 | 1.04x | 1.25x |
| tsv_wasm-json | 85.7 | 90.5 | 107.4 | 1.06x | 1.25x |
| tsv-internal | 254.0 | 293.2 | 303.9 | 1.15x | 1.20x |
| tsv_wasm-internal | 180.2 | 201.2 | 210.1 | 1.12x | 1.17x |

## format/css

| Impl | deno ops/sec | node ops/sec | bun ops/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 3.3 | 3.1 | 3.1 | 0.96x | 0.95x |
| tsv | 129.2 | 150.5 | 150.4 | 1.16x | 1.16x |
| tsv_wasm | 96.2 | 108.7 | 112.3 | 1.13x | 1.17x |
| oxfmt | 3.7 | 3.4 | 3.0 | 0.93x | 0.82x |
| biome-wasm | 15.8 | 18.2 | — | 1.15x | — |
