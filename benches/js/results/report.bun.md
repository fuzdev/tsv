# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-23T01:43:15.239Z — tsv 0.4.1 (23392a6e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.27       | 16  | 789.10   | 796.23   | 802.70   | 807.93   | 816.49   | 773.82   | 818.63   | baseline                     | baseline |
| tsv-json                    | 6.21       | 31  | 160.64   | 161.74   | 162.93   | 163.33   | 163.65   | 159.37   | 163.77   | 4.91x                        | 4.91x    |
| tsv-wasm-json               | 5.86       | 30  | 170.86   | 172.25   | 173.05   | 174.70   | 176.77   | 167.21   | 177.35   | 4.63x                        | 4.62x    |
| tsv-json-no-locations       | 8.59       | 43  | 116.46   | 117.58   | 118.11   | 118.18   | 119.45   | 112.94   | 119.92   | 6.79x                        | 6.78x    |
| tsv-wasm-json-no-locations  | 7.71       | 39  | 129.94   | 130.72   | 131.39   | 131.74   | 133.05   | 126.76   | 133.07   | 6.10x                        | 6.07x    |
| tsv-internal                | 55.18      | 248 | 18.12    | 18.15    | 18.26    | 18.38    | 18.72    | 18.03    | 18.90    | 43.6x                        | 43.5x    |
| tsv-wasm-internal           | 35.08      | 158 | 28.50    | 28.57    | 28.76    | 29.00    | 29.15    | 28.37    | 29.92    | 27.7x                        | 27.7x    |
| rsvelte-parse               | 2.04       | 10  | 489.46   | 491.62   | 492.31   | 499.21   | 504.73   | 486.09   | 506.11   | 1.62x                        | 1.61x    |
| rsvelte-parse-skip-expr-loc | 3.01       | 16  | 331.10   | 332.66   | 338.32   | 338.69   | 338.90   | 327.05   | 338.95   | 2.38x                        | 2.38x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.1 MB/s, tsv-json 15.0 MB/s, tsv-wasm-json 14.1 MB/s, tsv-json-no-locations 20.7 MB/s, tsv-wasm-json-no-locations 18.6 MB/s, tsv-internal 133.1 MB/s, tsv-wasm-internal 84.6 MB/s, rsvelte-parse 4.9 MB/s, rsvelte-parse-skip-expr-loc 7.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.9x tsv-internal, tsv-wasm-json 6.0x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.24       | 14 | 4.08    | 4.12    | 4.28    | 4.40    | 4.48    | 4.03    | 4.51    | baseline              | baseline |
| tsv        | 13.68      | 61 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 55.8x                 | 55.8x    |
| tsv-wasm   | 9.49       | 47 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 38.7x                 | 38.8x    |
| oxfmt      | 0.22       | 8  | 4.54    | 4.58    | 4.61    | —       | —       | 4.46    | 4.66    | 0.90x                 | 0.90x    |
| biome-wasm | 0.94       | 8  | 1.07    | 1.08    | 1.08    | —       | —       | 1.04    | 1.09    | 3.84x                 | 3.83x    |

**Files (intersection):** 945

**Throughput:** prettier 0.6 MB/s, tsv 31.8 MB/s, tsv-wasm 22.1 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript           | 0.18       | 15 | 5425.64  | 5440.05  | 5478.34  | 5505.16  | 5544.41  | 5363.99  | 5554.23  | baseline                      | baseline |
| tsv-json                   | 0.89       | 8  | 1129.37  | 1131.76  | 1134.38  | —        | —        | 1124.46  | 1136.28  | 4.80x                         | 4.80x    |
| tsv-wasm-json              | 0.89       | 6  | 1128.96  | 1143.33  | 1176.80  | —        | —        | 1125.83  | 1181.75  | 4.80x                         | 4.81x    |
| tsv-json-no-locations      | 1.56       | 8  | 641.63   | 647.68   | 648.19   | —        | —        | 634.35   | 648.52   | 8.44x                         | 8.46x    |
| tsv-wasm-json-no-locations | 1.46       | 8  | 684.12   | 686.95   | 688.43   | —        | —        | 674.03   | 688.73   | 7.94x                         | 7.93x    |
| tsv-internal               | 10.87      | 54 | 92.10    | 92.25    | 92.47    | 92.54    | 94.40    | 91.38    | 96.49    | 58.9x                         | 58.9x    |
| tsv-wasm-internal          | 7.19       | 26 | 139.14   | 139.88   | 140.09   | 140.28   | 140.46   | 138.86   | 140.50   | 39.0x                         | 39.0x    |
| oxc-parser                 | 1.14       | 8  | 881.84   | 883.96   | 885.77   | —        | —        | 872.18   | 889.74   | 6.15x                         | 6.15x    |
| oxc-parser-wasm            | 0.86       | 8  | 1167.07  | 1179.79  | 1191.25  | —        | —        | 1124.70  | 1198.21  | 4.66x                         | 4.65x    |
| yuku-parser                | 2.87       | 13 | 342.70   | 367.47   | 382.89   | 392.16   | 392.35   | 337.32   | 392.40   | 15.6x                         | 15.8x    |
| yuku-parser-wasm           | 3.53       | 13 | 283.60   | 295.72   | 301.78   | 308.38   | 330.50   | 277.58   | 336.03   | 19.2x                         | 19.1x    |
| swc                        | 0.74       | 8  | 1348.32  | 1348.92  | 1350.68  | —        | —        | 1344.38  | 1351.95  | 4.02x                         | 4.02x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.4 MB/s, tsv-json 16.3 MB/s, tsv-wasm-json 16.3 MB/s, tsv-json-no-locations 28.7 MB/s, tsv-wasm-json-no-locations 27.0 MB/s, tsv-internal 200.4 MB/s, tsv-wasm-internal 132.5 MB/s, oxc-parser 20.9 MB/s, oxc-parser-wasm 15.8 MB/s, yuku-parser 53.0 MB/s, yuku-parser-wasm 65.2 MB/s, swc 13.7 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.3x tsv-internal, tsv-wasm-json 8.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 14.07   | 14.19   | 14.28   | 14.34   | 14.35   | 13.71   | 14.35   | baseline              | baseline |
| tsv         | 2.45       | 13 | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 34.3x                 | 34.4x    |
| tsv-wasm    | 1.73       | 9  | 0.58    | 0.58    | 0.58    | —       | —       | 0.58    | 0.58    | 24.3x                 | 24.4x    |
| oxfmt       | 1.08       | 7  | 0.92    | 0.93    | 0.95    | —       | —       | 0.91    | 0.98    | 15.2x                 | 15.2x    |
| biome-wasm  | 0.23       | 8  | 4.35    | 4.38    | 4.40    | —       | —       | 4.30    | 4.41    | 3.22x                 | 3.23x    |
| dprint-wasm | 0.31       | 8  | 3.24    | 3.24    | 3.24    | —       | —       | 3.24    | 3.24    | 4.33x                 | 4.34x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.3 MB/s, tsv 45.2 MB/s, tsv-wasm 32.0 MB/s, oxfmt 20.0 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 49.12      | 233  | 20.35    | 20.53    | 20.70    | 20.96    | 23.15    | 19.90    | 23.58    | baseline                     | baseline |
| tsv-json          | 64.07      | 242  | 15.58    | 16.06    | 16.53    | 16.72    | 17.57    | 15.37    | 19.22    | 1.30x                        | 1.31x    |
| tsv-wasm-json     | 65.91      | 296  | 15.13    | 15.32    | 15.47    | 15.73    | 16.11    | 15.00    | 18.93    | 1.34x                        | 1.34x    |
| tsv-internal      | 290.50     | 1351 | 3.44     | 3.45     | 3.47     | 3.48     | 3.57     | 3.42     | 3.94     | 5.91x                        | 5.91x    |
| tsv-wasm-internal | 194.63     | 849  | 5.14     | 5.15     | 5.18     | 5.21     | 5.33     | 5.11     | 9.45     | 3.96x                        | 3.96x    |
| postcss           | 70.36      | 297  | 14.23    | 14.42    | 15.10    | 15.21    | 18.32    | 13.84    | 19.01    | 1.43x                        | 1.43x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.3 MB/s, tsv-json 25.2 MB/s, tsv-wasm-json 25.9 MB/s, tsv-internal 114.2 MB/s, tsv-wasm-internal 76.5 MB/s, postcss 27.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.5x tsv-internal, tsv-wasm-json 3.0x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.38       | 14  | 419.63   | 422.52   | 429.09   | 430.61   | 430.69   | 416.66   | 430.70   | baseline              | baseline |
| tsv        | 154.90     | 693 | 6.45     | 6.48     | 6.54     | 6.58     | 6.78     | 6.39     | 7.05     | 65.1x                 | 65.0x    |
| tsv-wasm   | 110.99     | 468 | 9.01     | 9.03     | 9.12     | 9.15     | 9.36     | 8.95     | 9.53     | 46.6x                 | 46.6x    |
| oxfmt      | 54.27      | 262 | 18.40    | 18.76    | 19.24    | 19.75    | 20.81    | 16.87    | 21.99    | 22.8x                 | 22.8x    |
| biome-wasm | 12.19      | 44  | 81.77    | 83.49    | 85.49    | 86.51    | 104.05   | 80.99    | 120.87   | 5.12x                 | 5.13x    |
| malva-wasm | 18.22      | 82  | 54.87    | 55.01    | 55.25    | 55.54    | 55.74    | 54.69    | 55.89    | 7.65x                 | 7.65x    |

**Files (intersection):** 54

**Throughput:** prettier 0.8 MB/s, tsv 54.8 MB/s, tsv-wasm 39.3 MB/s, oxfmt 19.2 MB/s, biome-wasm 4.3 MB/s, malva-wasm 6.4 MB/s

**Coverage:** prettier 55/55 (100%), tsv 55/55 (100%), tsv-wasm 55/55 (100%), oxfmt 55/55 (100%), biome-wasm 54/55 (98%), malva-wasm 55/55 (100%)

**Omitted from every row's timed set:** 1 of 55 files, 9.9% of the group's bytes (1.8% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.3 MB | 866.6 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 967.0 KB | 376.1 KB | 0.4x | 0.4x |
| tsv-wasm | 2.5 MB | 965.3 KB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 17.7x | 11.8x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.2x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.4x |
| tsv (ffi) | 3.4 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 674.3 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.0x | 2.7x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.4x | 2.1x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.7x | 4.3x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.7x | 7.2x |
| svelte + acorn-typescript parsers (js bundle) | 497.2 KB | 124.0 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 566.1 KB | 0.9x | 0.6x |
| prettier + parsers (js bundle) | 2.2 MB | 566.3 KB | 0.9x | 0.6x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **55.8x** prettier, **62.1x** oxfmt |
| format typescript (2642f) | **34.3x** prettier, **2.26x** oxfmt |
| format css (54f) | **65.1x** prettier, **2.85x** oxfmt |
| parse svelte (951f) | **4.91x** svelte/compiler, **3.04x** rsvelte-parse |
| parse typescript (2642f) | **4.80x** acorn-typescript, **0.78x** oxc-parser, **0.31x** yuku-parser, **1.19x** swc |
| parse css (55f) | **1.30x** svelte/compiler, **0.91x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **38.7x** prettier, **10.1x** biome-wasm |
| format typescript (2642f) | **24.3x** prettier, **7.55x** biome-wasm, **5.62x** dprint-wasm |
| format css (54f) | **46.6x** prettier, **9.11x** biome-wasm, **6.09x** malva-wasm |
| parse svelte (951f) | **4.63x** svelte/compiler |
| parse typescript (2642f) | **4.80x** acorn-typescript, **1.03x** oxc-parser-wasm, **0.25x** yuku-parser-wasm |
| parse css (55f) | **1.34x** svelte/compiler, **0.94x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are mechanism-matched; the payload is not: oxc’s default AST is span-only (`start`/`end`, no per-node `loc`, and no option to add one) where `tsv-json` carries the loc-bearing drop-in AST, so the payload-matched read is the `no-locations` line under each parse group. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and its payload-matched read is the same `no-locations` line), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

11 files skipped, 22 unique file+error combinations — Svelte 6, TypeScript 4, CSS 1 files.

**Per-benchmark skip counts:**
- format/svelte: biome-wasm: 6
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- format/typescript: biome-wasm: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2
- format/css: biome-wasm: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
