# tsv benchmark results — cross-runtime

**Generated:** 2026-09-21T19:22:29.434Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 939b51f8 @ 2026-09-21T18:02:28.380Z (tsv 0.4.1) — corpora b4c8d2862
- `node` 24.14.1: 939b51f8 @ 2026-09-21T18:21:14.710Z (tsv 0.4.1) — corpora b4c8d2862
- `bun` 1.4.2: 939b51f8 @ 2026-09-21T19:03:17.139Z (tsv 0.4.1) — corpora b4c8d2862
- `conformance` (node, coverage-only): 939b51f8 @ 2026-09-21T18:40:22.743Z (tsv 0.4.1)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 1 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/typescript/yuku-parser-wasm` deno (cv 3.3%, raw 10.4%, drift +0.3%, n=11). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 3 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/css/postcss` deno/node (1.4% vs 2.8% noise, n=399/333); `format/css/oxfmt` deno/bun (0.9% vs 3.7% noise, n=266/260); `format/css/biome-wasm` deno/node (1.4% vs 3.5% noise, n=46/37). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.6 | 1.6 | 1.2 | 0.96x | 0.75x |
| tsv-json | 4.2 | 3.8 | 6.1 | 0.90x | 1.47x |
| tsv-wasm-json | 3.5 | 3.5 | 5.9 | 0.99x | 1.68x |
| tsv-json-no-locations | 6.8 | 6.2 | 8.8 | 0.91x | 1.29x |
| tsv-wasm-json-no-locations | 5.5 | 5.5 | 8.0 | 1.02x | 1.46x |
| tsv-internal | 51.0 | 48.8 | 54.7 | 0.96x | 1.07x |
| tsv-wasm-internal | 30.1 | 33.7 | 35.2 | 1.12x | 1.17x |
| rsvelte-parse | 1.8 | 1.7 | 2.1 | 0.95x | 1.15x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.96x | 1.12x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.95x | 1.24x |
| tsv | 14.0 | 13.8 | 13.6 | 0.98x | 0.97x |
| tsv-wasm | 8.6 | 9.7 | 9.5 | 1.14x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.96x | 1.16x |
| biome-wasm | 1.1 | 0.9 | 0.9 | 0.78x | 0.85x |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.93x | 0.59x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.64x |
| tsv-wasm-json | 0.5 | 0.5 | 0.9 | 0.97x | 1.87x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.6 | 0.88x | 1.39x |
| tsv-wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.99x | 1.59x |
| tsv-internal | 9.5 | 8.7 | 10.6 | 0.92x | 1.12x |
| tsv-wasm-internal | 5.8 | 6.8 | 7.2 | 1.17x | 1.24x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.44x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.95x | 1.18x |
| yuku-parser | 2.1 | 2.3 | 2.9 | 1.10x | 1.35x |
| yuku-parser-wasm | 2.4 ⚠ | 2.7 | 3.5 | 1.15x ⚠ | 1.49x ⚠ |
| swc | 0.6 | 0.5 | 0.7 | 0.96x | 1.28x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.90x | 0.94x |
| tsv | 2.5 | 2.4 | 2.4 | 0.96x | 0.97x |
| tsv-wasm | 1.5 | 1.8 | 1.7 | 1.16x | 1.13x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.98x | 0.98x |
| biome-wasm | 0.2 | 0.2 | 0.2 | 0.93x | 1.04x |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.15x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 75.8 | 80.8 | 48.0 | 1.07x | 0.63x |
| tsv-json | 50.9 | 45.6 | 63.5 | 0.90x | 1.25x |
| tsv-wasm-json | 41.4 | 42.8 | 66.1 | 1.03x | 1.59x |
| tsv-internal | 276.6 | 254.2 | 288.5 | 0.92x | 1.04x |
| tsv-wasm-internal | 152.4 | 171.2 | 192.5 | 1.12x | 1.26x |
| postcss | 80.3 | 81.5 | 72.1 | 1.01x | 0.90x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.8 | 1.7 | 2.4 | 0.97x | 1.36x |
| tsv | 161.8 | 148.5 | 154.2 | 0.92x | 0.95x |
| tsv-wasm | 89.4 | 103.7 | 109.7 | 1.16x | 1.23x |
| oxfmt | 54.0 | 51.2 | 53.5 | 0.95x | 0.99x |
| biome-wasm | 10.2 | 10.0 | 12.0 | 0.99x | 1.18x |
| malva-wasm | 19.4 | 21.2 | 17.9 | 1.09x | 0.92x |
