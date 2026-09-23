# tsv benchmark results — cross-runtime

**Generated:** 2026-09-23T01:44:34.011Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 23392a6e @ 2026-09-23T01:06:46.440Z (tsv 0.4.1) — corpora b4c8d2862
- `node` 24.14.1: 23392a6e @ 2026-09-23T01:25:34.842Z (tsv 0.4.1) — corpora b4c8d2862
- `bun` 1.4.2: 23392a6e @ 2026-09-23T01:43:15.239Z (tsv 0.4.1) — corpora b4c8d2862
- `conformance` (node, coverage-only): 23392a6e @ 2026-09-23T01:44:33.674Z (tsv 0.4.1)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Within noise:** 4 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/css/postcss` deno/node (1.3% vs 2.9% noise, n=409/370); `format/css/oxfmt` deno/node (0.3% vs 3.5% noise, n=257/257); `format/css/oxfmt` deno/bun (3.1% vs 3.6% noise, n=257/262); `format/css/oxfmt` node/bun (3.4% vs 3.8% noise, n=257/262). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.7 | 1.6 | 1.3 | 0.95x | 0.76x |
| tsv-json | 4.2 | 3.7 | 6.2 | 0.89x | 1.48x |
| tsv-wasm-json | 3.6 | 3.5 | 5.9 | 0.98x | 1.64x |
| tsv-json-no-locations | 6.8 | 6.2 | 8.6 | 0.90x | 1.26x |
| tsv-wasm-json-no-locations | 5.5 | 5.5 | 7.7 | 1.01x | 1.40x |
| tsv-internal | 50.9 | 49.2 | 55.2 | 0.97x | 1.08x |
| tsv-wasm-internal | 30.4 | 33.2 | 35.1 | 1.09x | 1.15x |
| rsvelte-parse | 1.8 | 1.7 | 2.0 | 0.94x | 1.14x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.94x | 1.10x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.93x | 1.25x |
| tsv | 13.9 | 13.8 | 13.7 | 0.99x | 0.98x |
| tsv-wasm | 8.5 | 9.7 | 9.5 | 1.14x | 1.12x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.95x | 1.16x |
| biome-wasm | 1.1 | 0.9 | 0.9 | 0.77x | 0.85x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.92x | 0.60x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.65x |
| tsv-wasm-json | 0.5 | 0.5 | 0.9 | 0.96x | 1.86x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.6 | 0.87x | 1.39x |
| tsv-wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.98x | 1.57x |
| tsv-internal | 9.7 | 8.8 | 10.9 | 0.91x | 1.12x |
| tsv-wasm-internal | 5.8 | 6.7 | 7.2 | 1.14x | 1.23x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.89x | 1.42x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.96x | 1.18x |
| yuku-parser | 2.1 | 2.4 | 2.9 | 1.11x | 1.35x |
| yuku-parser-wasm | 2.4 | 2.7 | 3.5 | 1.16x | 1.50x |
| swc | 0.6 | 0.5 | 0.7 | 0.94x | 1.27x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.94x |
| tsv | 2.5 | 2.4 | 2.4 | 0.95x | 0.96x |
| tsv-wasm | 1.5 | 1.8 | 1.7 | 1.15x | 1.14x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.96x | 0.99x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.93x | 1.05x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 75.8 | 82.9 | 49.1 | 1.09x | 0.65x |
| tsv-json | 51.3 | 46.0 | 64.1 | 0.90x | 1.25x |
| tsv-wasm-json | 41.6 | 42.8 | 65.9 | 1.03x | 1.58x |
| tsv-internal | 276.0 | 255.6 | 290.5 | 0.93x | 1.05x |
| tsv-wasm-internal | 152.6 | 167.0 | 194.6 | 1.09x | 1.28x |
| postcss | 81.7 | 82.8 | 70.4 | 1.01x | 0.86x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.7 | 2.4 | 0.94x | 1.30x |
| tsv | 161.9 | 149.8 | 154.9 | 0.93x | 0.96x |
| tsv-wasm | 89.2 | 104.0 | 111.0 | 1.17x | 1.24x |
| oxfmt | 52.6 | 52.5 | 54.3 | 1.00x | 1.03x |
| biome-wasm | 10.1 | 10.4 | 12.2 | 1.03x | 1.21x |
| malva-wasm | 19.5 | 21.2 | 18.2 | 1.09x | 0.94x |
