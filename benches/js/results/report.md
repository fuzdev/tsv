# tsv benchmark results — cross-runtime

**Generated:** 2026-09-15T01:31:43.604Z

**Runtimes:** deno, node, bun — each runtime’s full report is its `report.<runtime>.{json,md}` sibling.

- `deno` 2.9.6: b2f9c39e @ 2026-09-15T01:07:21.584Z (tsv 0.3.0) — corpora e8d37ffc1
- `node` 24.14.1: b2f9c39e @ 2026-09-15T01:20:12.180Z (tsv 0.3.0) — corpora e8d37ffc1
- `bun` 1.4.2: b2f9c39e @ 2026-09-15T01:30:23.499Z (tsv 0.3.0) — corpora e8d37ffc1
- `conformance` (node, coverage-only): b2f9c39e @ 2026-09-15T01:31:43.272Z (tsv 0.3.0)

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64

**Not measured everywhere:** bun — biome-wasm. The implementation behind each row failed to load on the runtime(s) named, so it contributes no measurement there — a row thinner than its neighbours, or missing outright, is a load failure rather than a speed result. The per-runtime report’s `unavailable` carries the impl and the cause.

**Unstable:** 7 per-runtime measurement(s) were not stable, so every ratio through them is unreadable — `parse/svelte/tsv-json` bun (cv 3.6%, raw 4.0%, drift -5.5%, n=29); `parse/svelte/tsv-json-no-locations` bun (cv 4.1%, raw 4.1%, drift -6.9%, n=42); `parse/svelte/tsv_wasm-json-no-locations` bun (cv 4.4%, raw 4.4%, drift -7.5%, n=38); `parse/typescript/yuku-parser` deno (cv 4.8%, raw 7.8%, drift -5.1%, n=10); `format/typescript/biome-wasm` node (cv 6.8%, raw 38.3%, drift +179.7%, n=5); `parse/css/tsv-json` bun (cv 6.1%, raw 6.1%, drift -10.6%, n=290); `parse/css/tsv_wasm-json` bun (cv 5.4%, raw 5.6%, drift -9.9%, n=287). The cell is marked `⚠` in its table. A drift is a cost that moved WHILE the row was measured (the median of the second half of its timings against the first’s); the cleaned cv cannot see it, and a longer window moves such a row’s answer rather than converging it. Re-run the runtime before reading the row, and read the per-runtime report’s §Unstable Rows for the row’s own detail.

**Within noise:** 9 per-runtime delta(s) are smaller than the two measurements' combined variation, so they are not runtime effects — `parse/svelte/tsv_wasm-json` deno/node (0.6% vs 0.7% noise, n=14/15); `format/svelte/tsv` deno/node (1.3% vs 1.7% noise, n=57/46); `format/svelte/tsv` deno/bun (0.6% vs 2.0% noise, n=57/56); `format/svelte/tsv` node/bun (0.7% vs 1.3% noise, n=46/56); `parse/css/svelte/compiler` deno/node (4.6% vs 4.6% noise, n=445/409); `parse/css/postcss` deno/node (0.8% vs 4.2% noise, n=395/377); `format/css/oxfmt` deno/node (1.2% vs 4.9% noise, n=244/242); `format/css/oxfmt` deno/bun (0.7% vs 5.1% noise, n=244/243); `format/css/oxfmt` node/bun (1.9% vs 5.0% noise, n=242/243). Read those cells as "no difference". The two cv values behind each are `entries[].cv` in the per-runtime JSON — NOT that report's §Unstable Rows, which lists only rows past its own 10% threshold and so names none of these: a cell lands here whenever the delta is small relative to the noise, which two perfectly ordinary 3% rows satisfy. `n` is the cleaned timings behind each cv — a row under 10 a side is left unclassified rather than called quiet on an estimate that thin. Every pair of runtimes is classified, not only each against the ratio base, and a row named under **Unstable** above is never classified here.

A per-runtime delta on the same row is the signal: same engine, different runtime + binding boundary (Deno → FFI, Node/Bun → N-API). Ratios are vs `deno` (> 1 = faster than deno). A group (or row) flagged `⚠ files …` iterated *different per-runtime intersections* (each runtime times the files all its impls passed preflight on), so a sliver of the ratio can be file-set difference rather than runtime effect.

## parse/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 1.6 | 1.6 | 1.2 | 0.97x | 0.72x |
| tsv-json | 4.1 | 3.7 | 5.9 ⚠ | 0.90x | 1.43x ⚠ |
| tsv_wasm-json | 3.5 | 3.5 | 5.6 | 0.99x | 1.60x |
| tsv-json-no-locations | 6.6 | 6.1 | 8.3 ⚠ | 0.92x | 1.25x ⚠ |
| tsv_wasm-json-no-locations | 5.3 | 5.5 | 7.4 ⚠ | 1.03x | 1.39x ⚠ |
| tsv-internal | 47.0 | 45.6 | 50.0 | 0.97x | 1.06x |
| tsv_wasm-internal | 28.4 | 32.2 | 32.6 | 1.13x | 1.15x |
| rsvelte-parse | 1.8 | 1.7 | 2.0 | 0.95x | 1.13x |
| rsvelte-parse-skip-expr-loc | 2.7 | 2.6 | 3.0 | 0.95x | 1.09x |

## format/svelte

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.2 | 0.2 | 0.2 | 0.97x | 1.21x |
| tsv | 12.5 | 12.7 | 12.6 | 1.01x | 1.01x |
| tsv_wasm | 7.7 | 9.0 | 8.7 | 1.16x | 1.13x |
| oxfmt | 0.2 | 0.2 | 0.2 | 0.95x | 1.13x |
| biome-wasm | 1.1 | 0.8 | — | 0.78x | — |

## parse/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| acorn-typescript | 0.3 | 0.3 | 0.2 | 0.93x | 0.64x |
| tsv-json | 0.5 | 0.5 | 0.9 | 0.88x | 1.64x |
| tsv_wasm-json | 0.5 | 0.5 | 0.9 | 0.98x | 1.88x |
| tsv-json-no-locations | 1.1 | 1.0 | 1.5 | 0.88x | 1.37x |
| tsv_wasm-json-no-locations | 0.9 | 0.9 | 1.5 | 0.99x | 1.58x |
| tsv-internal | 8.7 | 8.4 | 9.9 | 0.97x | 1.14x |
| tsv_wasm-internal | 5.8 | 6.5 | 6.9 | 1.13x | 1.19x |
| oxc-parser | 0.8 | 0.7 | 1.1 | 0.90x | 1.41x |
| oxc-parser-wasm | 0.7 | 0.7 | 0.8 | 0.95x | 1.15x |
| yuku-parser | 2.1 ⚠ | 2.3 | 2.8 | 1.12x ⚠ | 1.35x ⚠ |
| yuku-parser-wasm | 2.3 | 2.7 | 3.4 | 1.17x | 1.46x |
| swc | 0.6 | 0.6 | 0.7 | 0.95x | 1.24x |

## format/typescript

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 0.1 | 0.1 | 0.1 | 0.86x | 0.85x |
| tsv | 2.3 | 2.2 | 2.2 | 0.96x | 0.97x |
| tsv_wasm | 1.4 | 1.6 | 1.6 | 1.15x | 1.12x |
| oxfmt | 1.1 | 1.1 | 1.1 | 0.97x | 0.96x |
| biome-wasm | 0.2 | 0.1 ⚠ | — | 0.35x ⚠ | — |
| dprint-wasm | 0.3 | 0.3 | 0.3 | 1.13x | 1.16x |

## parse/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| svelte/compiler | 89.9 | 85.8 | 48.9 | 0.95x | 0.54x |
| tsv-json | 49.1 | 43.8 | 57.8 ⚠ | 0.89x | 1.18x ⚠ |
| tsv_wasm-json | 39.0 | 40.7 | 57.5 ⚠ | 1.04x | 1.47x ⚠ |
| tsv-internal | 228.2 | 215.8 | 242.4 | 0.95x | 1.06x |
| tsv_wasm-internal | 127.5 | 146.7 | 152.0 | 1.15x | 1.19x |
| postcss | 80.2 | 79.5 | 59.6 | 0.99x | 0.74x |

## format/css

| Impl | deno sweeps/sec | node sweeps/sec | bun sweeps/sec | node/deno | bun/deno |
| --- | ---: | ---: | ---: | ---: | ---: |
| prettier | 1.6 | 1.5 | 1.8 | 0.93x | 1.10x |
| tsv | 130.2 | 121.5 | 126.6 | 0.93x | 0.97x |
| tsv_wasm | 73.1 | 85.3 | 88.2 | 1.17x | 1.21x |
| oxfmt | 49.1 | 48.5 | 49.4 | 0.99x | 1.01x |
| biome-wasm | 9.8 | 6.4 | — | 0.65x | — |
| malva-wasm | 17.5 | 19.0 | 16.3 | 1.08x | 0.93x |
