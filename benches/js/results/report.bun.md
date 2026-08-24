# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.0

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T15:47:31.275Z — tsv 0.2.0 (831e5193)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.58       | 7   | 632.03   | 639.77   | 661.72   | —        | —        | 622.19   | 680.74   | baseline                     |
| tsv-json                    | 7.00       | 27  | 142.53   | 147.40   | 160.82   | 174.48   | 175.37   | 139.17   | 175.73   | 4.43x                        |
| tsv_wasm-json               | 6.68       | 29  | 148.82   | 152.47   | 166.44   | 178.83   | 179.56   | 143.63   | 179.79   | 4.23x                        |
| tsv-json-no-locations       | 9.53       | 46  | 103.34   | 110.10   | 112.85   | 118.93   | 122.90   | 99.32    | 125.05   | 6.02x                        |
| tsv_wasm-json-no-locations  | 8.90       | 41  | 110.66   | 117.13   | 123.47   | 128.71   | 131.75   | 106.94   | 132.60   | 5.63x                        |
| tsv-internal                | 52.63      | 253 | 18.83    | 19.45    | 19.77    | 19.91    | 20.28    | 18.50    | 20.35    | 33.3x                        |
| tsv_wasm-internal           | 36.34      | 152 | 27.37    | 28.09    | 28.27    | 28.35    | 28.71    | 27.22    | 29.13    | 23.0x                        |
| rsvelte-parse               | 3.28       | 12  | 305.82   | 320.19   | 338.38   | 343.10   | 345.48   | 299.64   | 346.08   | 2.07x                        |
| rsvelte-parse-skip-expr-loc | 5.20       | 26  | 191.40   | 200.00   | 201.26   | 202.81   | 203.75   | 183.58   | 203.95   | 3.29x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 3.1 MB/s, tsv-json 13.7 MB/s, tsv_wasm-json 13.1 MB/s, tsv-json-no-locations 18.7 MB/s, tsv_wasm-json-no-locations 17.5 MB/s, tsv-internal 103.2 MB/s, tsv_wasm-internal 71.3 MB/s, rsvelte-parse 6.4 MB/s, rsvelte-parse-skip-expr-loc 10.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.5x tsv-internal, tsv_wasm-json 5.4x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.29       | 5  | 3.41    | 3.50    | 3.52    | —       | —       | 3.38    | 3.53    | baseline              |
| tsv       | 13.04      | 63 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.07    | 0.08    | 44.9x                 |
| tsv_wasm  | 9.44       | 42 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 32.5x                 |
| oxfmt     | 0.27       | 5  | 3.71    | 3.77    | 3.79    | —       | —       | 3.63    | 3.80    | 0.93x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.6 MB/s, tsv 25.6 MB/s, tsv_wasm 18.5 MB/s, oxfmt 0.5 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.20       | 5  | 4978.14  | 4981.28  | 4982.48  | —        | —        | 4951.67  | 4983.27  | baseline                      |
| tsv-json                   | 0.83       | 5  | 1203.41  | 1205.21  | 1205.47  | —        | —        | 1199.84  | 1205.64  | 4.13x                         |
| tsv_wasm-json              | 0.85       | 5  | 1170.02  | 1174.54  | 1177.57  | —        | —        | 1166.78  | 1179.59  | 4.24x                         |
| tsv-json-no-locations      | 1.45       | 8  | 689.77   | 694.07   | 702.46   | —        | —        | 678.05   | 702.48   | 7.20x                         |
| tsv_wasm-json-no-locations | 1.38       | 7  | 724.77   | 728.75   | 733.13   | —        | —        | 710.60   | 734.62   | 6.87x                         |
| tsv-internal               | 8.06       | 31 | 124.15   | 124.71   | 127.32   | 127.45   | 127.61   | 123.57   | 127.65   | 40.1x                         |
| tsv_wasm-internal          | 5.82       | 24 | 171.64   | 172.58   | 175.81   | 176.23   | 177.31   | 170.46   | 177.63   | 28.9x                         |
| oxc-parser                 | 1.15       | 5  | 871.34   | 877.51   | 881.93   | —        | —        | 868.92   | 884.39   | 5.70x                         |
| oxc-parser-wasm            | 0.84       | 5  | 1190.89  | 1210.55  | 1224.38  | —        | —        | 1141.26  | 1233.61  | 4.17x                         |
| yuku-parser                | 2.99       | 13 | 333.55   | 345.99   | 378.71   | 402.40   | 424.07   | 322.76   | 429.48   | 14.9x                         |
| yuku-parser-wasm           | 3.71       | 18 | 266.50   | 279.16   | 289.85   | 297.86   | 317.32   | 256.99   | 322.18   | 18.4x                         |
| swc                        | 0.74       | 5  | 1345.01  | 1347.54  | 1349.01  | —        | —        | 1336.23  | 1349.99  | 3.70x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 3.6 MB/s, tsv-json 14.9 MB/s, tsv_wasm-json 15.3 MB/s, tsv-json-no-locations 26.0 MB/s, tsv_wasm-json-no-locations 24.9 MB/s, tsv-internal 144.9 MB/s, tsv_wasm-internal 104.7 MB/s, oxc-parser 20.6 MB/s, oxc-parser-wasm 15.1 MB/s, yuku-parser 53.8 MB/s, yuku-parser-wasm 66.6 MB/s, swc 13.4 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.7x tsv-internal, tsv_wasm-json 6.8x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ----------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier    | 0.07       | 6 | 13895.25 | 13910.23 | 13931.80 | —        | —        | 13860.77 | 13959.18 | baseline              |
| tsv         | 1.73       | 7 | 577.76   | 578.45   | 582.98   | —        | —        | 574.39   | 584.01   | 24.1x                 |
| tsv_wasm    | 1.30       | 6 | 767.19   | 768.16   | 770.98   | —        | —        | 764.00   | 774.98   | 18.1x                 |
| oxfmt       | 1.16       | 6 | 862.31   | 864.61   | 867.71   | —        | —        | 846.99   | 870.05   | 16.2x                 |
| dprint-wasm | 0.32       | 5 | 3148.48  | 3149.88  | 3153.52  | —        | —        | 3144.08  | 3155.95  | 4.42x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.3 MB/s, tsv 31.2 MB/s, tsv_wasm 23.5 MB/s, oxfmt 20.9 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 65.07      | 238  | 15.45    | 15.74    | 22.86    | 23.61    | 23.97    | 14.55    | 24.51    | baseline                     |
| tsv-json          | 71.84      | 358  | 13.89    | 14.52    | 14.94    | 15.15    | 15.91    | 12.84    | 19.32    | 1.10x                        |
| tsv_wasm-json     | 74.17      | 369  | 13.33    | 14.10    | 14.48    | 14.59    | 14.99    | 12.57    | 18.01    | 1.14x                        |
| tsv-internal      | 329.68     | 1334 | 3.03     | 3.06     | 3.14     | 3.32     | 3.46     | 2.99     | 4.80     | 5.07x                        |
| tsv_wasm-internal | 223.82     | 919  | 4.46     | 4.51     | 4.60     | 4.77     | 4.92     | 4.43     | 5.54     | 3.44x                        |
| postcss           | 85.91      | 347  | 11.53    | 12.46    | 14.05    | 18.47    | 19.71    | 10.34    | 20.96    | 1.32x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 21.8 MB/s, tsv-json 24.1 MB/s, tsv_wasm-json 24.8 MB/s, tsv-internal 110.4 MB/s, tsv_wasm-internal 74.9 MB/s, postcss 28.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.6x tsv-internal, tsv_wasm-json 3.0x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.54       | 11  | 396.74   | 404.74   | 441.50   | 469.67   | 492.85   | 383.94   | 498.65   | baseline              |
| tsv        | 145.24     | 671 | 6.87     | 6.95     | 7.06     | 7.20     | 7.39     | 6.76     | 8.41     | 57.2x                 |
| tsv_wasm   | 103.83     | 434 | 9.61     | 9.74     | 9.85     | 9.89     | 10.10    | 9.52     | 10.87    | 40.9x                 |
| oxfmt      | 58.26      | 284 | 17.17    | 17.59    | 18.00    | 18.27    | 20.01    | 15.34    | 21.87    | 22.9x                 |
| malva-wasm | 18.90      | 95  | 52.67    | 53.49    | 53.92    | 54.09    | 54.63    | 51.99    | 54.67    | 7.44x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.9 MB/s, tsv 48.6 MB/s, tsv_wasm 34.8 MB/s, oxfmt 19.5 MB/s, malva-wasm 6.3 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 839.8 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 928.7 KB | 361.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 932.2 KB | — | — |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.2x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 673.9 KB | 200.3 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.5 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 657.5 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.8x |
| oxc-parser (napi) | 2.1 MB | 885.7 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.0 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 741.1 KB | 310.4 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.3 MB | 3.3 MB | 2.2x | 2.0x |
| rsvelte compiler (napi) | 14.5 MB | 6.0 MB | 3.8x | 3.6x |
| swc (napi) | 31.9 MB | 11.9 MB | 8.3x | 7.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **44.9x** prettier, **48.4x** oxfmt |
| format typescript (2504f) | **24.1x** prettier, **1.49x** oxfmt |
| format css (49f) | **57.2x** prettier, **2.49x** oxfmt |
| parse svelte (773f) | **4.43x** svelte/compiler, **2.14x** rsvelte-parse |
| parse typescript (2503f) | **4.13x** acorn-typescript, **0.72x** oxc-parser, **0.28x** yuku-parser, **1.12x** swc |
| parse css (49f) | **1.10x** svelte/compiler, **0.84x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **32.5x** prettier |
| format typescript (2504f) | **18.1x** prettier, **4.11x** dprint-wasm |
| format css (49f) | **40.9x** prettier, **5.49x** malva-wasm |
| parse svelte (773f) | **4.23x** svelte/compiler |
| parse typescript (2503f) | **4.24x** acorn-typescript, **1.02x** oxc-parser-wasm, **0.23x** yuku-parser-wasm |
| parse css (49f) | **1.14x** svelte/compiler, **0.86x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (the axis a throughput ratio integrates; per component the spread is wider), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

3 files skipped, 12 unique file+error combinations — Svelte 0, TypeScript 3, CSS 0 files.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
