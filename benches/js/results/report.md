# tsv benchmark results — cross-runtime

**Generated:** 2026-10-02T17:53:31.809Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 120567d4 @ 2026-10-02T17:53:31.615Z (tsv 0.5.0) — corpora 446268fab
- `node` 24.14.1: 120567d4 @ 2026-10-02T17:05:50.124Z (tsv 0.5.0) — corpora 446268fab
- `bun` 1.4.2: 120567d4 @ 2026-10-02T17:23:19.689Z (tsv 0.5.0) — corpora 446268fab
- `conformance` (node, coverage-only): 120567d4 @ 2026-10-02T17:24:37.255Z (tsv 0.5.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Within noise:** 7 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `format/svelte/tsv-wasm` node/bun (0.1% vs 0.5% noise, n=42/49); `format/typescript/tsv` node/bun (0.0% vs 0.3% noise, n=13/13); `format/typescript/tsv-wasm` node/bun (0.1% vs 0.2% noise, n=10/10); `parse/css/postcss` deno/node (0.2% vs 2.9% noise, n=418/321); `format/css/oxfmt` deno/bun (0.5% vs 4.3% noise, n=263/263); `format/css/oxfmt` node/bun (3.9% vs 4.6% noise, n=254/263); `format/css/biome-wasm` deno/node (1.2% vs 4.8% noise, n=45/39). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.6 | 1.6 | 1.3 | 0.96x | 0.77x |
| tsv-json | 4.3 | 3.9 | 6.5 | 0.89x | 1.49x |
| tsv-wasm-json | 3.7 | 3.7 | 6.5 | 0.98x | 1.73x |
| tsv-json-no-locations | 7.1 | 6.5 | 9.5 | 0.91x | 1.32x |
| tsv-wasm-json-no-locations | 5.8 | 5.9 | 8.8 | 1.01x | 1.50x |
| tsv-internal | 62.2 | 59.6 | 68.1 | 0.96x | 1.09x |
| tsv-wasm-internal | 35.7 | 40.4 | 42.7 | 1.13x | 1.20x |
| rsvelte-parse | 1.8 | 1.7 | 2.1 | 0.96x | 1.15x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.1 | 0.96x | 1.12x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.96x | 1.26x |
| tsv | 15.0 | 14.8 | 14.6 | 0.99x | 0.97x |
| tsv-wasm | 9.3 | 10.4 | 10.3 | 1.12x | 1.12x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.96x | 1.18x |
| biome-wasm | 1.1 | 0.9 | 0.9 | 0.77x | 0.85x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.95x | 0.61x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.68x |
| tsv-wasm-json | 0.5 | 0.5 | 1.0 | 0.96x | 1.94x |
| tsv-json-no-locations | 1.2 | 1.0 | 1.6 | 0.87x | 1.40x |
| tsv-wasm-json-no-locations | 1.0 | 1.0 | 1.6 | 0.98x | 1.61x |
| tsv-internal | 11.2 | 9.8 | 12.2 | 0.88x | 1.09x |
| tsv-wasm-internal | 6.4 | 7.3 | 8.1 | 1.14x | 1.27x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.43x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.96x | 1.20x |
| yuku-parser | 2.1 | 2.4 | 2.9 | 1.12x | 1.36x |
| yuku-parser-wasm | 2.3 | 2.7 | 3.5 | 1.17x | 1.51x |
| swc | 0.6 | 0.6 | 0.7 | 0.95x | 1.27x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.86x | 0.96x |
| tsv | 2.7 | 2.6 | 2.6 | 0.95x | 0.95x |
| tsv-wasm | 1.6 | 1.8 | 1.8 | 1.13x | 1.13x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.99x | 1.01x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.93x | 1.05x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 75.9 | 82.3 | 49.4 | 1.08x | 0.65x |
| tsv-json | 54.9 | 48.7 | 70.4 | 0.89x | 1.28x |
| tsv-wasm-json | 46.4 | 47.1 | 77.9 | 1.01x | 1.68x |
| tsv-internal | 397.2 | 364.5 | 438.9 | 0.92x | 1.11x |
| tsv-wasm-internal | 216.6 | 242.2 | 299.6 | 1.12x | 1.38x |
| postcss | 83.5 | 83.3 | 71.7 | 1.00x | 0.86x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.7 | 2.4 | 0.95x | 1.30x |
| tsv | 218.4 | 200.6 | 210.8 | 0.92x | 0.97x |
| tsv-wasm | 120.3 | 139.1 | 154.0 | 1.16x | 1.28x |
| oxfmt | 54.4 | 52.1 | 54.1 | 0.96x | 1.00x |
| biome-wasm | 10.1 | 10.0 | 12.2 | 0.99x | 1.21x |
| malva-wasm | 19.6 | 21.2 | 17.9 | 1.08x | 0.92x |
