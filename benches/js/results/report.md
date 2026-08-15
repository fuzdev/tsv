# tsv benchmark results — cross-runtime

**Generated:** 2026-08-15T16:27:34.962Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.3: a51ae509 @ 2026-07-31T23:33:39.072Z (tsv 0.2.0)
- `node` 24.14.1: d063479a @ 2026-07-22T09:18:34.995Z (tsv 0.2.0)
- `bun` 1.3.14: d063479a @ 2026-07-22T09:27:00.825Z (tsv 0.2.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

⚠ **Mixed vintages** — the sibling reports above come from different commits/versions, so the cross-runtime ratios are unreliable; re-run the stale runtimes (`deno task bench:perf` refreshes all three).

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

⚠ files deno 765 / node 767 / bun 767

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 2.2 | 2.2 | 1.4 | 1.00x | 0.64x |
| tsv-json | 5.0 | 4.6 | 6.0 | 0.92x | 1.20x |
| tsv-json-no-locations | 7.9 | 7.3 | 8.4 | 0.93x | 1.07x |
| tsv_wasm-json | 4.3 | 4.2 | 5.6 | 0.98x | 1.32x |
| tsv_wasm-json-no-locations | 6.3 | 6.4 | 7.6 | 1.02x | 1.20x |
| tsv-internal | 50.5 | 49.2 | 52.8 | 0.97x | 1.05x |
| tsv_wasm-internal | 32.9 | 36.2 | 37.7 | 1.10x | 1.15x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier ⚠ files deno 765 / node 767 / bun 767 | 0.2 | 0.2 | 0.3 | 1.03x | 1.17x |
| tsv ⚠ files deno 765 / node 767 / bun 767 | 13.9 | 14.7 | 14.5 | 1.06x | 1.04x |
| tsv_wasm ⚠ files deno 765 / node 767 / bun 767 | 8.8 | 10.3 | 10.5 | 1.16x | 1.19x |
| oxfmt ⚠ files deno 765 / node 767 / bun 767 | 0.2 | 0.2 | 0.2 | 1.02x | 0.87x |
| biome-wasm ⚠ files deno 765 / node 767 / bun — | 1.3 | 1.1 | — | 0.82x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript ⚠ files deno 2454 / node 2445 / bun 2445 | 0.3 | 0.3 | 0.1 | 0.96x | 0.44x |
| tsv-json ⚠ files deno 2454 / node 2445 / bun 2445 | 0.5 | 0.5 | 0.7 | 0.89x | 1.31x |
| tsv-json-no-locations ⚠ files deno 2454 / node 2445 / bun 2445 | 1.1 | 1.0 | 1.3 | 0.88x | 1.11x |
| tsv_wasm-json ⚠ files deno 2454 / node 2445 / bun 2445 | 0.5 | 0.5 | 0.7 | 0.95x | 1.39x |
| tsv_wasm-json-no-locations ⚠ files deno 2454 / node 2445 / bun 2445 | 0.9 | 0.9 | 1.2 | 0.97x | 1.22x |
| tsv-internal ⚠ files deno 2454 / node 2445 / bun 2445 | 7.8 | 7.1 | 8.3 | 0.92x | 1.07x |
| tsv_wasm-internal ⚠ files deno 2454 / node 2445 / bun 2445 | 5.2 | 5.6 | 5.9 | 1.08x | 1.14x |
| oxc-parser ⚠ files deno 2454 / node 2445 / bun 2445 | 0.8 | 0.8 | 1.0 | 0.94x | 1.26x |
| oxc-parser-wasm ⚠ files deno 2454 / node 2445 / bun — | 0.7 | 0.7 | — | 0.99x | — |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier ⚠ files deno 2455 / node 2446 / bun 2446 | 0.1 | 0.1 | 0.1 | 0.95x | 0.82x |
| tsv ⚠ files deno 2455 / node 2446 / bun 2446 | 2.0 | 2.0 | 1.9 | 1.00x | 0.99x |
| tsv_wasm ⚠ files deno 2455 / node 2446 / bun 2446 | 1.3 | 1.4 | 1.5 | 1.14x | 1.17x |
| oxfmt ⚠ files deno 2455 / node 2446 / bun 2446 | 1.2 | 1.2 | 1.0 | 1.03x | 0.84x |
| biome-wasm ⚠ files deno 2455 / node 2446 / bun — | 0.2 | 0.2 | — | 0.97x | — |
| dprint-wasm ⚠ files deno 2455 / node 2446 / bun 2446 | 0.3 | 0.3 | 0.3 | 1.15x | 1.20x |

## parse/css

⚠ files deno 49 / node 50 / bun 50

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 94.5 | 109.6 | 68.2 | 1.16x | 0.72x |
| tsv-json | 66.9 | 57.7 | 74.5 | 0.86x | 1.11x |
| tsv_wasm-json | 52.4 | 55.3 | 69.7 | 1.05x | 1.33x |
| tsv-internal | 328.4 | 301.6 | 343.2 | 0.92x | 1.05x |
| tsv_wasm-internal | 185.2 | 211.1 | 218.4 | 1.14x | 1.18x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier ⚠ files deno 49 / node 50 / bun 50 | 1.9 | 1.8 | 2.0 | 0.94x | 1.03x |
| tsv ⚠ files deno 49 / node 50 / bun 50 | 151.6 | 147.6 | 153.8 | 0.97x | 1.01x |
| tsv_wasm ⚠ files deno 49 / node 50 / bun 50 | 89.0 | 102.7 | 104.1 | 1.15x | 1.17x |
| oxfmt ⚠ files deno 49 / node 50 / bun 50 | 53.6 | 52.9 | 46.7 | 0.99x | 0.87x |
| biome-wasm ⚠ files deno 49 / node 50 / bun — | 9.8 | 7.5 | — | 0.77x | — |
