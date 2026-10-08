# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.7

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-08T13:39:45.082Z — tsv 0.6.0 (b24d1494)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 2.1% at the 95th percentile and 4.7% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 3.05       | 47  | 327.49   | 331.04   | 333.58   | 333.98   | 336.95   | 321.18   | 337.21   | baseline                     | baseline |
| tsv                         | 7.65       | 111 | 130.68   | 131.21   | 131.86   | 132.64   | 145.08   | 129.60   | 151.96   | 2.51x                        | 2.51x    |
| tsv-wasm                    | 6.22       | 88  | 160.74   | 161.44   | 162.06   | 165.83   | 173.84   | 159.38   | 179.76   | 2.04x                        | 2.04x    |
| tsv+locations               | 5.44       | 84  | 183.59   | 184.53   | 185.35   | 185.90   | 186.62   | 181.94   | 187.39   | 1.78x                        | 1.78x    |
| tsv-wasm+locations          | 4.64       | 71  | 215.75   | 216.70   | 217.20   | 217.77   | 218.23   | 213.06   | 218.65   | 1.52x                        | 1.52x    |
| tsv-internal                | 62.46      | 804 | 16.01    | 16.06    | 16.26    | 16.43    | 16.59    | 15.90    | 16.92    | 20.5x                        | 20.5x    |
| tsv-wasm-internal           | 36.71      | 492 | 27.25    | 27.34    | 27.47    | 27.68    | 27.89    | 27.03    | 28.16    | 12.0x                        | 12.0x    |
| rsvelte-parse               | 1.80       | 27  | 553.96   | 555.18   | 556.41   | 557.00   | 557.36   | 551.37   | 557.42   | 0.59x                        | 0.59x    |
| rsvelte-parse-skip-expr-loc | 2.74       | 42  | 364.06   | 366.23   | 367.83   | 368.06   | 369.26   | 360.41   | 369.74   | 0.90x                        | 0.90x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 7.4 MB/s, tsv 18.4 MB/s, tsv-wasm 15.0 MB/s, tsv+locations 13.1 MB/s, tsv-wasm+locations 11.2 MB/s, tsv-internal 150.6 MB/s, tsv-wasm-internal 88.5 MB/s, rsvelte-parse 4.4 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.2x tsv-internal, tsv-wasm 5.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.20       | 24  | 5068.32  | 5091.90  | 5133.08  | 5142.21  | 5148.93  | 4994.23  | 5150.64  | baseline              | baseline |
| tsv        | 14.94      | 214 | 66.89    | 67.23    | 67.80    | 68.41    | 69.05    | 66.33    | 69.48    | 75.7x                 | 75.8x    |
| tsv-wasm   | 9.22       | 139 | 108.63   | 109.24   | 109.62   | 109.97   | 110.29   | 106.64   | 110.48   | 46.7x                 | 46.7x    |
| oxfmt      | 0.20       | 24  | 5100.92  | 5106.50  | 5119.73  | 5121.82  | 5138.77  | 5059.95  | 5143.82  | 0.99x                 | 0.99x    |
| biome-wasm | 1.08       | 24  | 927.44   | 931.75   | 934.69   | 936.74   | 939.08   | 914.11   | 939.68   | 5.47x                 | 5.46x    |

**Files (intersection):** 923

**Throughput:** prettier 0.5 MB/s, tsv 34.8 MB/s, tsv-wasm 21.5 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript   | 0.96       | 24  | 1035.98  | 1047.57  | 1050.59  | 1052.17  | 1052.64  | 1029.07  | 1052.72  | baseline                      | baseline |
| tsv                | 1.27       | 23  | 788.60   | 789.76   | 790.77   | 791.77   | 792.81   | 786.26   | 793.08   | 1.32x                         | 1.31x    |
| tsv-wasm           | 1.06       | 21  | 947.75   | 950.83   | 952.94   | 956.99   | 962.82   | 942.63   | 964.37   | 1.10x                         | 1.09x    |
| tsv+locations      | 1.00       | 23  | 997.34   | 1001.14  | 1002.88  | 1003.65  | 1006.75  | 991.74   | 1007.64  | 1.04x                         | 1.04x    |
| tsv-wasm+locations | 0.86       | 21  | 1156.30  | 1161.14  | 1164.99  | 1166.81  | 1170.06  | 1152.65  | 1170.94  | 0.90x                         | 0.90x    |
| tsv-internal       | 10.94      | 155 | 91.36    | 91.61    | 92.01    | 92.30    | 92.96    | 90.73    | 93.38    | 11.4x                         | 11.3x    |
| tsv-wasm-internal  | 6.36       | 96  | 157.64   | 158.87   | 159.37   | 159.67   | 159.93   | 154.12   | 160.11   | 6.61x                         | 6.57x    |
| oxc-parser         | 0.82       | 24  | 1216.16  | 1221.13  | 1225.61  | 1226.85  | 1232.97  | 1193.39  | 1234.76  | 0.86x                         | 0.85x    |
| oxc-parser-wasm    | 0.74       | 22  | 1349.14  | 1352.05  | 1358.35  | 1359.99  | 1362.35  | 1336.79  | 1362.98  | 0.77x                         | 0.77x    |
| yuku-parser        | 2.30       | 30  | 434.19   | 450.28   | 454.77   | 461.15   | 466.25   | 426.99   | 466.50   | 2.39x                         | 2.39x    |
| yuku-parser-wasm   | 2.64       | 32  | 380.30   | 384.69   | 399.60   | 402.04   | 413.87   | 368.19   | 419.76   | 2.74x                         | 2.72x    |
| swc                | 0.62       | 24  | 1606.79  | 1620.27  | 1628.06  | 1628.74  | 1639.38  | 1592.69  | 1642.54  | 0.65x                         | 0.64x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 17.7 MB/s, tsv 23.4 MB/s, tsv-wasm 19.5 MB/s, tsv+locations 18.5 MB/s, tsv-wasm+locations 15.9 MB/s, tsv-internal 201.6 MB/s, tsv-wasm-internal 117.2 MB/s, oxc-parser 15.2 MB/s, oxc-parser-wasm 13.7 MB/s, yuku-parser 42.3 MB/s, yuku-parser-wasm 48.7 MB/s, swc 11.4 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.6x tsv-internal, tsv-wasm 6.0x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 24 | 13.70   | 13.75   | 13.80   | 13.84   | 13.90   | 13.48   | 13.91   | baseline              | baseline |
| tsv         | 2.62       | 42 | 0.38    | 0.38    | 0.38    | 0.38    | 0.39    | 0.38    | 0.39    | 35.8x                 | 35.8x    |
| tsv-wasm    | 1.59       | 21 | 0.63    | 0.63    | 0.63    | 0.63    | 0.64    | 0.62    | 0.64    | 21.8x                 | 21.8x    |
| oxfmt       | 1.18       | 23 | 0.85    | 0.85    | 0.86    | 0.86    | 0.86    | 0.83    | 0.86    | 16.2x                 | 16.2x    |
| biome-wasm  | 0.23       | 23 | 4.37    | 4.40    | 4.42    | 4.43    | 4.43    | 4.35    | 4.43    | 3.13x                 | 3.13x    |
| dprint-wasm | 0.27       | 24 | 3.73    | 3.74    | 3.75    | 3.75    | 3.75    | 3.72    | 3.75    | 3.67x                 | 3.67x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.3 MB/s, tsv 48.2 MB/s, tsv-wasm 29.3 MB/s, oxfmt 21.8 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 4.9 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 98.71      | 1472 | 10.12    | 10.26    | 10.48    | 10.62    | 10.95    | 9.73     | 13.80    | baseline                     | baseline |
| tsv                | 61.11      | 881  | 16.36    | 16.50    | 16.60    | 16.75    | 17.23    | 16.07    | 20.84    | 0.62x                        | 0.62x    |
| tsv-wasm           | 49.52      | 723  | 20.19    | 20.31    | 20.41    | 20.55    | 20.95    | 19.68    | 22.05    | 0.50x                        | 0.50x    |
| tsv+locations      | 47.99      | 593  | 20.81    | 21.34    | 22.51    | 22.75    | 23.40    | 20.29    | 32.28    | 0.49x                        | 0.49x    |
| tsv-wasm+locations | 40.87      | 479  | 24.49    | 25.05    | 26.19    | 26.39    | 26.70    | 23.94    | 33.61    | 0.41x                        | 0.41x    |
| tsv-internal       | 398.44     | 5688 | 2.51     | 2.52     | 2.54     | 2.57     | 2.65     | 2.48     | 3.82     | 4.04x                        | 4.04x    |
| tsv-wasm-internal  | 218.13     | 3029 | 4.59     | 4.64     | 4.66     | 4.68     | 4.79     | 4.48     | 8.52     | 2.21x                        | 2.21x    |
| postcss            | 80.86      | 1205 | 12.32    | 12.58    | 12.88    | 13.00    | 13.45    | 11.76    | 14.25    | 0.82x                        | 0.82x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 38.8 MB/s, tsv 24.0 MB/s, tsv-wasm 19.5 MB/s, tsv+locations 18.9 MB/s, tsv-wasm+locations 16.1 MB/s, tsv-internal 156.6 MB/s, tsv-wasm-internal 85.7 MB/s, postcss 31.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.5x tsv-internal, tsv-wasm 4.4x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.82       | 28   | 548.17   | 560.18   | 565.42   | 566.52   | 577.37   | 525.26   | 581.38   | baseline              | baseline |
| tsv        | 219.00     | 2879 | 4.57     | 4.58     | 4.64     | 4.72     | 4.81     | 4.53     | 5.14     | 120.2x                | 120.0x   |
| tsv-wasm   | 121.04     | 1568 | 8.27     | 8.32     | 8.39     | 8.46     | 8.64     | 8.16     | 10.81    | 66.4x                 | 66.3x    |
| oxfmt      | 55.22      | 808  | 18.06    | 18.43    | 18.83    | 19.10    | 19.89    | 16.86    | 20.82    | 30.3x                 | 30.3x    |
| biome-wasm | 10.57      | 128  | 94.72    | 95.93    | 97.18    | 101.35   | 103.26   | 93.29    | 103.63   | 5.80x                 | 5.79x    |
| malva-wasm | 19.54      | 250  | 51.19    | 51.32    | 51.63    | 51.85    | 52.14    | 51.00    | 52.34    | 10.7x                 | 10.7x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 77.5 MB/s, tsv-wasm 42.8 MB/s, oxfmt 19.5 MB/s, biome-wasm 3.7 MB/s, malva-wasm 6.9 MB/s

**Coverage:** prettier 55/55 (100%), tsv 55/55 (100%), tsv-wasm 55/55 (100%), oxfmt 55/55 (100%), biome-wasm 54/55 (98%), malva-wasm 55/55 (100%)

**Omitted from every row's timed set:** 1 of 55 files, 9.9% of the group's bytes (1.8% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.4 MB | 948.1 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.0 MB | 397.4 KB | 0.4x | 0.4x |
| tsv-wasm | 2.7 MB | 1.0 MB | — | — |
| biome (wasm) | 46.9 MB | 11.9 MB | 17.4x | 11.4x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 698.9 KB | 202.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.6 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.9 MB | 4.5 MB | 3.1x | 2.7x |
| tsv format (ffi) | 3.3 MB | 1.5 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 698.7 KB | 0.4x | 0.4x |
| tsv (napi) | 4.0 MB | 1.8 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 880.8 KB | 0.6x | 0.5x |
| oxfmt (napi) | 8.8 MB | 3.7 MB | 2.5x | 2.2x |
| yuku-parser (napi) | 681.1 KB | 286.0 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 2.5x | 2.2x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 5.0x | 4.5x |
| swc (napi) | 10.5 MB | 10.2 MB | 3.0x | 6.1x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **75.7x** prettier, **76.1x** oxfmt |
| format typescript (2593f) | **35.8x** prettier, **2.21x** oxfmt |
| format css (54f) | **120.2x** prettier, **3.97x** oxfmt |
| parse svelte (929f) | **2.51x** svelte/compiler, **4.24x** rsvelte-parse |
| parse typescript (2593f) | **1.32x** acorn-typescript, **1.54x** oxc-parser, **0.55x** yuku-parser, **2.04x** swc |
| parse css (55f) | **0.62x** svelte/compiler, **0.76x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **46.7x** prettier, **8.55x** biome-wasm |
| format typescript (2593f) | **21.8x** prettier, **6.97x** biome-wasm, **5.94x** dprint-wasm |
| format css (54f) | **66.4x** prettier, **11.4x** biome-wasm, **6.19x** malva-wasm |
| parse svelte (929f) | **2.04x** svelte/compiler |
| parse typescript (2593f) | **1.10x** acorn-typescript, **1.42x** oxc-parser-wasm, **0.40x** yuku-parser-wasm |
| parse css (55f) | **0.50x** svelte/compiler, **0.61x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.41x, wasm 1.34x | typescript: native 1.27x, wasm 1.22x | css: native 1.27x, wasm 1.21x

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
