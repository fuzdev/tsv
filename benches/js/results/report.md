# tsv benchmark results — cross-runtime

**Generated:** 2026-09-15T03:00:03.872Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: 9b06410e @ 2026-09-15T02:37:12.152Z (tsv 0.3.0) — corpora e8d37ffc1
- `node` 24.14.1: 9b06410e @ 2026-09-15T02:48:31.241Z (tsv 0.3.0) — corpora e8d37ffc1
- `bun` 1.4.2: 9b06410e @ 2026-09-15T02:58:45.083Z (tsv 0.3.0) — corpora e8d37ffc1
- `conformance` (node, coverage-only): 9b06410e @ 2026-09-15T03:00:03.558Z (tsv 0.3.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Unstable:** 8 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/svelte/tsv_wasm-json` bun (cv 1.2%, raw 4.9%, drift -9.1%, n=21); `parse/svelte/tsv-json-no-locations` bun (cv 4.3%, raw 4.3%, drift -7.3%, n=41); `parse/svelte/tsv_wasm-json-no-locations` bun (cv 4.9%, raw 4.9%, drift -8.2%, n=38); `format/svelte/biome-wasm` bun (cv 1.9%, raw 8.2%, drift -6.4%, n=4); `parse/typescript/yuku-parser` bun (cv 3.4%, raw 4.6%, drift -5.8%, n=14); `format/typescript/biome-wasm` bun (cv 0.8%, raw 5.7%, drift -6.7%, n=4); `parse/css/tsv-json` bun (cv 5.7%, raw 6.0%, drift -9.0%, n=287); `parse/css/tsv_wasm-json` bun (cv 4.2%, raw 5.7%, drift -9.3%, n=258). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s — negative: it got faster, still warming up; positive: it got slower, degrading); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 6 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `format/svelte/tsv` deno/node (0.0% vs 1.0% noise, n=53/50); `parse/css/svelte/compiler` deno/node (3.2% vs 4.0% noise, n=451/435); `parse/css/postcss` deno/node (3.4% vs 3.7% noise, n=416/393); `format/css/oxfmt` deno/node (1.9% vs 4.2% noise, n=242/240); `format/css/oxfmt` deno/bun (0.6% vs 4.3% noise, n=242/245); `format/css/oxfmt` node/bun (2.5% vs 4.3% noise, n=240/245). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.6 | 1.6 | 1.1 | 0.97x | 0.71x |
| tsv-json | 4.1 | 3.7 | 5.9 | 0.89x | 1.42x |
| tsv_wasm-json | 3.5 | 3.5 | 5.9 ⚠ | 0.98x | 1.67x ⚠ |
| tsv-json-no-locations | 6.7 | 6.1 | 8.2 ⚠ | 0.91x | 1.22x ⚠ |
| tsv_wasm-json-no-locations | 5.4 | 5.5 | 7.5 ⚠ | 1.02x | 1.39x ⚠ |
| tsv-internal | 47.3 | 45.6 | 50.4 | 0.97x | 1.07x |
| tsv_wasm-internal | 28.5 | 32.5 | 32.8 | 1.14x | 1.15x |
| rsvelte-parse | 1.8 | 1.7 | 2.0 | 0.94x | 1.11x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 2.9 | 0.95x | 1.08x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.94x | 1.25x |
| tsv | 12.6 | 12.6 | 12.5 | 1.00x | 0.99x |
| tsv_wasm | 7.8 | 9.0 | 8.7 | 1.14x | 1.11x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.96x | 1.19x |
| biome-wasm | 1.1 | 0.9 | 0.9 ⚠ | 0.79x | 0.83x ⚠ |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.93x | 0.70x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.87x | 1.64x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.96x | 1.89x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.5 | 0.88x | 1.37x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.99x | 1.57x |
| tsv-internal | 9.1 | 8.4 | 10.0 | 0.92x | 1.10x |
| tsv_wasm-internal | 5.7 | 6.5 | 6.9 | 1.15x | 1.20x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.43x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.9 | 0.96x | 1.24x |
| yuku-parser | 2.1 | 2.4 | 2.9 ⚠ | 1.11x | 1.37x ⚠ |
| yuku-parser-wasm | 2.3 | 2.7 | 3.4 | 1.18x | 1.47x |
| swc | 0.6 | 0.6 | 0.7 | 0.94x | 1.26x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.88x | 0.93x |
| tsv | 2.3 | 2.2 | 2.2 | 0.97x | 0.98x |
| tsv_wasm | 1.4 | 1.6 | 1.6 | 1.15x | 1.13x |
| oxfmt | 1.1 | 1.1 | 1.1 | 1.01x | 1.03x |
| biome-wasm | 0.2 | 0.2 | 0.2 ⚠ | 0.93x | 1.03x ⚠ |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.12x | 1.16x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 90.3 | 87.4 | 50.5 | 0.97x | 0.56x |
| tsv-json | 48.5 | 43.8 | 58.1 ⚠ | 0.90x | 1.20x ⚠ |
| tsv_wasm-json | 39.0 | 40.8 | 58.9 ⚠ | 1.05x | 1.51x ⚠ |
| tsv-internal | 228.0 | 216.4 | 242.5 | 0.95x | 1.06x |
| tsv_wasm-internal | 128.7 | 148.0 | 154.4 | 1.15x | 1.20x |
| postcss | 83.8 | 81.0 | 70.7 | 0.97x | 0.84x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.6 | 1.5 | 2.1 | 0.93x | 1.28x |
| tsv | 130.4 | 122.1 | 127.5 | 0.94x | 0.98x |
| tsv_wasm | 73.6 | 86.5 | 88.3 | 1.18x | 1.20x |
| oxfmt | 49.1 | 48.1 | 49.3 | 0.98x | 1.01x |
| biome-wasm | 9.6 | 8.7 | 11.4 | 0.90x | 1.19x |
| malva-wasm | 17.5 | 19.1 | 16.6 | 1.10x | 0.95x |
