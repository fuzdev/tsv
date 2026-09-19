# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-19T23:04:29.425Z — tsv 0.4.1 (8837350f)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.25       | 16  | 789.96   | 823.82   | 834.78   | 838.97   | 844.13   | 764.24   | 845.42   | baseline                     | baseline |
| tsv-json                    | 6.20       | 30  | 161.17   | 161.86   | 162.60   | 163.42   | 164.75   | 159.28   | 165.00   | 4.95x                        | 4.90x    |
| tsv-wasm-json               | 6.03       | 29  | 165.60   | 166.46   | 168.35   | 169.30   | 170.25   | 163.23   | 170.60   | 4.82x                        | 4.77x    |
| tsv-json-no-locations       | 8.92       | 43  | 112.07   | 112.58   | 113.56   | 114.17   | 115.96   | 111.06   | 116.95   | 7.12x                        | 7.05x    |
| tsv-wasm-json-no-locations  | 8.12       | 39  | 123.21   | 123.97   | 124.73   | 125.23   | 130.56   | 121.43   | 131.59   | 6.48x                        | 6.41x    |
| tsv-internal                | 54.90      | 242 | 18.22    | 18.25    | 18.35    | 18.49    | 18.81    | 18.15    | 18.96    | 43.8x                        | 43.4x    |
| tsv-wasm-internal           | 35.78      | 157 | 27.95    | 27.99    | 28.25    | 28.47    | 28.65    | 27.84    | 28.71    | 28.6x                        | 28.3x    |
| rsvelte-parse               | 2.05       | 10  | 485.11   | 489.85   | 491.49   | 497.77   | 502.79   | 483.79   | 504.04   | 1.64x                        | 1.63x    |
| rsvelte-parse-skip-expr-loc | 3.06       | 12  | 327.27   | 332.76   | 339.90   | 340.61   | 340.90   | 324.50   | 340.98   | 2.44x                        | 2.41x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.0 MB/s, tsv-json 15.0 MB/s, tsv-wasm-json 14.6 MB/s, tsv-json-no-locations 21.5 MB/s, tsv-wasm-json-no-locations 19.6 MB/s, tsv-internal 132.4 MB/s, tsv-wasm-internal 86.3 MB/s, rsvelte-parse 5.0 MB/s, rsvelte-parse-skip-expr-loc 7.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.8x tsv-internal, tsv-wasm-json 5.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.23       | 14 | 4.36    | 4.41    | 4.55    | 4.65    | 4.73    | 4.29    | 4.76    | baseline              | baseline |
| tsv        | 13.66      | 59 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 59.6x                 | 59.6x    |
| tsv-wasm   | 9.53       | 44 | 0.10    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 41.6x                 | 41.6x    |
| oxfmt      | 0.21       | 6  | 4.73    | 4.75    | 4.78    | —       | —       | 4.66    | 4.84    | 0.92x                 | 0.92x    |
| biome-wasm | 0.94       | 8  | 1.08    | 1.08    | 1.08    | —       | —       | 1.04    | 1.09    | 4.09x                 | 4.05x    |

**Files (intersection):** 945

**Throughput:** prettier 0.5 MB/s, tsv 31.8 MB/s, tsv-wasm 22.2 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript           | 0.19       | 16 | 5291.21  | 5320.07  | 5340.19  | 5353.33  | 5380.89  | 5248.84  | 5387.78  | baseline                      | baseline |
| tsv-json                   | 0.90       | 8  | 1113.87  | 1114.71  | 1116.02  | —        | —        | 1108.79  | 1118.05  | 4.76x                         | 4.75x    |
| tsv-wasm-json              | 0.91       | 8  | 1096.33  | 1100.05  | 1107.50  | —        | —        | 1091.89  | 1108.02  | 4.83x                         | 4.83x    |
| tsv-json-no-locations      | 1.55       | 8  | 645.24   | 648.82   | 652.17   | —        | —        | 634.41   | 652.47   | 8.22x                         | 8.20x    |
| tsv-wasm-json-no-locations | 1.49       | 8  | 669.69   | 674.77   | 676.13   | —        | —        | 664.98   | 677.11   | 7.90x                         | 7.90x    |
| tsv-internal               | 10.73      | 53 | 93.25    | 93.49    | 93.92    | 94.04    | 95.85    | 92.42    | 97.35    | 56.9x                         | 56.7x    |
| tsv-wasm-internal          | 7.39       | 37 | 135.19   | 135.55   | 135.90   | 136.16   | 136.25   | 134.60   | 136.25   | 39.2x                         | 39.1x    |
| oxc-parser                 | 1.14       | 8  | 873.54   | 874.37   | 876.38   | —        | —        | 870.02   | 876.72   | 6.07x                         | 6.06x    |
| oxc-parser-wasm            | 0.86       | 8  | 1157.43  | 1180.46  | 1187.35  | —        | —        | 1126.38  | 1193.98  | 4.57x                         | 4.57x    |
| yuku-parser                | 2.94       | 12 | 339.21   | 362.18   | 380.08   | 389.44   | 390.52   | 332.22   | 390.79   | 15.6x                         | 15.6x    |
| yuku-parser-wasm           | 3.55       | 16 | 280.60   | 289.22   | 300.04   | 309.63   | 330.03   | 273.90   | 335.13   | 18.8x                         | 18.9x    |
| swc                        | 0.73       | 7  | 1372.68  | 1373.71  | 1374.01  | —        | —        | 1368.15  | 1374.33  | 3.86x                         | 3.85x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.5 MB/s, tsv-json 16.6 MB/s, tsv-wasm-json 16.8 MB/s, tsv-json-no-locations 28.6 MB/s, tsv-wasm-json-no-locations 27.5 MB/s, tsv-internal 197.8 MB/s, tsv-wasm-internal 136.3 MB/s, oxc-parser 21.1 MB/s, oxc-parser-wasm 15.9 MB/s, yuku-parser 54.1 MB/s, yuku-parser-wasm 65.4 MB/s, swc 13.4 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.9x tsv-internal, tsv-wasm-json 8.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.06       | 16 | 15.56   | 15.61   | 15.63   | 15.64   | 15.65   | 15.48   | 15.66   | baseline              | baseline |
| tsv         | 2.45       | 13 | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 38.2x                 | 38.2x    |
| tsv-wasm    | 1.75       | 8  | 0.57    | 0.57    | 0.57    | —       | —       | 0.57    | 0.58    | 27.2x                 | 27.2x    |
| oxfmt       | 1.08       | 8  | 0.92    | 0.94    | 0.95    | —       | —       | 0.90    | 0.97    | 16.8x                 | 16.9x    |
| biome-wasm  | 0.23       | 8  | 4.36    | 4.38    | 4.40    | —       | —       | 4.32    | 4.40    | 3.57x                 | 3.57x    |
| dprint-wasm | 0.31       | 7  | 3.26    | 3.26    | 3.26    | —       | —       | 3.25    | 3.26    | 4.78x                 | 4.78x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.2 MB/s, tsv 45.2 MB/s, tsv-wasm 32.2 MB/s, oxfmt 19.9 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 48.92      | 231  | 20.47    | 20.59    | 20.74    | 21.20    | 23.43    | 19.88    | 23.83    | baseline                     | baseline |
| tsv-json          | 63.97      | 283  | 15.55    | 16.00    | 16.42    | 16.80    | 17.27    | 15.31    | 20.23    | 1.31x                        | 1.32x    |
| tsv-wasm-json     | 67.13      | 303  | 14.89    | 14.97    | 15.14    | 15.52    | 16.14    | 14.73    | 19.31    | 1.37x                        | 1.37x    |
| tsv-internal      | 289.24     | 1361 | 3.46     | 3.47     | 3.48     | 3.50     | 3.56     | 3.43     | 3.90     | 5.91x                        | 5.92x    |
| tsv-wasm-internal | 197.12     | 891  | 5.07     | 5.08     | 5.10     | 5.12     | 5.19     | 5.05     | 5.42     | 4.03x                        | 4.03x    |
| postcss           | 61.26      | 249  | 16.32    | 16.64    | 17.45    | 17.61    | 19.94    | 15.94    | 22.29    | 1.25x                        | 1.25x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.8 MB/s, tsv-json 25.9 MB/s, tsv-wasm-json 27.2 MB/s, tsv-internal 117.2 MB/s, tsv-wasm-internal 79.8 MB/s, postcss 24.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.5x tsv-internal, tsv-wasm-json 2.9x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.04       | 15  | 487.15   | 495.29   | 498.52   | 500.73   | 502.14   | 483.45   | 502.49   | baseline              | baseline |
| tsv        | 155.06     | 698 | 6.45     | 6.47     | 6.54     | 6.58     | 6.76     | 6.38     | 7.02     | 75.9x                 | 75.6x    |
| tsv-wasm   | 112.02     | 471 | 8.93     | 8.95     | 9.03     | 9.07     | 9.31     | 8.88     | 9.67     | 54.8x                 | 54.6x    |
| oxfmt      | 54.19      | 264 | 18.42    | 18.82    | 19.17    | 19.60    | 21.12    | 17.02    | 22.50    | 26.5x                 | 26.4x    |
| biome-wasm | 12.20      | 47  | 81.65    | 82.96    | 83.74    | 84.14    | 84.54    | 80.77    | 84.81    | 5.97x                 | 5.97x    |
| malva-wasm | 18.22      | 91  | 55.01    | 55.10    | 55.39    | 55.80    | 56.62    | 54.33    | 57.53    | 8.91x                 | 8.86x    |

**Files (intersection):** 54

**Throughput:** prettier 0.7 MB/s, tsv 56.4 MB/s, tsv-wasm 40.8 MB/s, oxfmt 19.7 MB/s, biome-wasm 4.4 MB/s, malva-wasm 6.6 MB/s

**Coverage:** prettier 55/55 (100%), tsv 55/55 (100%), tsv-wasm 55/55 (100%), oxfmt 55/55 (100%), biome-wasm 54/55 (98%), malva-wasm 55/55 (100%)

**Omitted from every row's timed set:** 1 of 55 files, 10.2% of the group's bytes (1.8% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.3 MB | 867.2 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 968.9 KB | 377.0 KB | 0.4x | 0.4x |
| tsv-wasm | 2.5 MB | 966.4 KB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 17.6x | 11.8x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.2x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.4x |
| tsv (ffi) | 3.4 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 675.4 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.0x | 2.7x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.4x | 2.1x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.7x | 4.3x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.7x | 7.2x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **59.6x** prettier, **64.5x** oxfmt |
| format typescript (2642f) | **38.2x** prettier, **2.28x** oxfmt |
| format css (54f) | **75.9x** prettier, **2.86x** oxfmt |
| parse svelte (951f) | **4.95x** svelte/compiler, **3.02x** rsvelte-parse |
| parse typescript (2642f) | **4.76x** acorn-typescript, **0.78x** oxc-parser, **0.31x** yuku-parser, **1.23x** swc |
| parse css (55f) | **1.31x** svelte/compiler, **1.04x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **41.6x** prettier, **10.2x** biome-wasm |
| format typescript (2642f) | **27.2x** prettier, **7.62x** biome-wasm, **5.69x** dprint-wasm |
| format css (54f) | **54.8x** prettier, **9.18x** biome-wasm, **6.15x** malva-wasm |
| parse svelte (951f) | **4.82x** svelte/compiler |
| parse typescript (2642f) | **4.83x** acorn-typescript, **1.06x** oxc-parser-wasm, **0.26x** yuku-parser-wasm |
| parse css (55f) | **1.37x** svelte/compiler, **1.10x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are mechanism-matched; the payload is not: oxc’s default AST is span-only (`start`/`end`, no per-node `loc`, and no option to add one) where `tsv-json` carries the loc-bearing drop-in AST, so the payload-matched read is the `no-locations` line under each parse group. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and its payload-matched read is the same `no-locations` line), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/svelte/svelte/compiler | 3.3% | 3.3% | -6.1% | 16/16 |

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
