# tsv benchmark results — cross-runtime

**Generated:** 2026-09-14T22:33:09.442Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 7eb42464 @ 2026-09-14T22:11:04.153Z (tsv 0.3.0) — corpora e8d37ffc1
- `node` 24.14.1: 7eb42464 @ 2026-09-14T22:22:31.255Z (tsv 0.3.0) — corpora e8d37ffc1
- `bun` 1.4.2: 7eb42464 @ 2026-09-14T22:31:51.671Z (tsv 0.3.0) — corpora e8d37ffc1
- `conformance` (node, coverage-only): 7eb42464 @ 2026-09-14T22:33:09.128Z (tsv 0.3.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — biome-wasm. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Within noise:** 5 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `format/svelte/tsv` node (0.1% vs 2.0% noise, n=56/58); `format/svelte/tsv` bun (0.1% vs 2.0% noise, n=56/58); `parse/css/svelte/compiler` node (3.9% vs 4.4% noise, n=443/411); `parse/css/postcss` node (0.7% vs 4.1% noise, n=399/382); `format/css/oxfmt` bun (0.2% vs 4.9% noise, n=242/237). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.6 | 1.6 | 1.2 | 0.96x | 0.73x |
| tsv-json | 4.2 | 3.7 | 6.0 | 0.88x | 1.44x |
| tsv_wasm-json | 3.5 | 3.5 | 5.7 | 0.98x | 1.61x |
| tsv-json-no-locations | 6.7 | 6.1 | 8.3 | 0.91x | 1.24x |
| tsv_wasm-json-no-locations | 5.4 | 5.5 | 7.5 | 1.02x | 1.39x |
| tsv-internal | 47.4 | 46.1 | 50.7 | 0.97x | 1.07x |
| tsv_wasm-internal | 28.4 | 32.1 | 32.5 | 1.13x | 1.15x |
| rsvelte-parse | 1.8 | 1.7 | 2.0 | 0.95x | 1.13x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.95x | 1.08x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.95x | 1.15x |
| tsv | 12.6 | 12.6 | 12.6 | 1.00x | 1.00x |
| tsv_wasm | 7.8 | 9.0 | 8.7 | 1.14x | 1.12x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.96x | 1.11x |
| biome-wasm | 1.1 | 0.9 | — | 0.78x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.94x | 0.67x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.63x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.97x | 1.87x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.5 | 0.89x | 1.38x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.99x | 1.58x |
| tsv-internal | 9.1 | 8.4 | 10.0 | 0.93x | 1.10x |
| tsv_wasm-internal | 5.7 | 6.5 | 7.0 | 1.14x | 1.23x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.40x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.98x | 1.22x |
| yuku-parser | 2.1 | 2.4 | 2.8 | 1.11x | 1.33x |
| yuku-parser-wasm | 2.3 | 2.7 | 3.4 | 1.16x | 1.47x |
| swc | 0.6 | 0.6 | 0.7 | 0.96x | 1.28x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.89x | 0.86x |
| tsv | 2.3 | 2.2 | 2.3 | 0.97x | 0.98x |
| tsv_wasm | 1.4 | 1.6 | 1.6 | 1.17x | 1.14x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.99x | 0.99x |
| biome-wasm | 0.2 | 0.2 | — | 0.73x | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 90.0 | 86.5 | 48.3 | 0.96x | 0.54x |
| tsv-json | 48.7 | 44.0 | 57.4 | 0.90x | 1.18x |
| tsv_wasm-json | 39.0 | 40.6 | 57.3 | 1.04x | 1.47x |
| tsv-internal | 228.5 | 214.7 | 240.1 | 0.94x | 1.05x |
| tsv_wasm-internal | 127.3 | 146.1 | 154.3 | 1.15x | 1.21x |
| postcss | 81.0 | 80.5 | 62.4 | 0.99x | 0.77x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.6 | 1.5 | 1.8 | 0.93x | 1.15x |
| tsv | 130.8 | 121.2 | 126.6 | 0.93x | 0.97x |
| tsv_wasm | 72.9 | 86.1 | 87.4 | 1.18x | 1.20x |
| oxfmt | 48.3 | 46.1 | 48.4 | 0.96x | 1.00x |
| biome-wasm | 9.8 | 6.3 | — | 0.64x | — |
| malva-wasm | 17.4 | 19.1 | 16.3 | 1.09x | 0.94x |
