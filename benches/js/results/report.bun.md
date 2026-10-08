# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-08T15:05:59.589Z — tsv 0.6.0 (b24d1494)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.7% apart at the median, 3.9% at the 95th percentile and 6.0% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.73       | 42  | 365.62   | 371.82   | 378.29   | 385.69   | 392.45   | 350.92   | 395.90   | baseline                     | baseline |
| tsv                         | 9.60       | 144 | 104.00   | 105.20   | 106.05   | 106.73   | 107.74   | 101.81   | 108.32   | 3.52x                        | 3.52x    |
| tsv-wasm                    | 8.92       | 130 | 111.73   | 113.39   | 114.33   | 114.93   | 116.49   | 109.39   | 117.23   | 3.27x                        | 3.27x    |
| tsv+locations               | 6.73       | 99  | 148.22   | 149.82   | 151.27   | 151.63   | 152.66   | 145.42   | 153.51   | 2.47x                        | 2.47x    |
| tsv-wasm+locations          | 6.37       | 93  | 156.77   | 157.86   | 158.96   | 159.87   | 163.62   | 153.56   | 165.43   | 2.34x                        | 2.33x    |
| tsv-internal                | 69.70      | 933 | 14.35    | 14.42    | 14.54    | 14.66    | 14.92    | 14.20    | 15.60    | 25.5x                        | 25.5x    |
| tsv-wasm-internal           | 43.38      | 587 | 23.07    | 23.14    | 23.32    | 23.52    | 23.86    | 22.84    | 24.76    | 15.9x                        | 15.9x    |
| rsvelte-parse               | 2.01       | 32  | 497.04   | 498.75   | 499.91   | 500.53   | 502.52   | 492.88   | 503.41   | 0.74x                        | 0.74x    |
| rsvelte-parse-skip-expr-loc | 2.99       | 45  | 334.82   | 336.03   | 337.06   | 337.92   | 338.72   | 329.40   | 339.18   | 1.10x                        | 1.09x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 6.6 MB/s, tsv 23.1 MB/s, tsv-wasm 21.5 MB/s, tsv+locations 16.2 MB/s, tsv-wasm+locations 15.4 MB/s, tsv-internal 168.1 MB/s, tsv-wasm-internal 104.6 MB/s, rsvelte-parse 4.9 MB/s, rsvelte-parse-skip-expr-loc 7.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.3x tsv-internal, tsv-wasm 4.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.23       | 24  | 4307.24  | 4366.94  | 4394.18  | 4435.37  | 4443.33  | 4126.53  | 4443.55  | baseline              | baseline |
| tsv        | 15.05      | 225 | 66.38    | 66.67    | 67.01    | 67.24    | 67.58    | 65.79    | 67.90    | 64.8x                 | 64.9x    |
| tsv-wasm   | 10.54      | 158 | 94.84    | 95.54    | 95.99    | 96.37    | 97.07    | 93.36    | 97.21    | 45.4x                 | 45.4x    |
| oxfmt      | 0.23       | 24  | 4388.39  | 4415.33  | 4476.45  | 4488.33  | 4506.68  | 4298.54  | 4511.78  | 0.98x                 | 0.98x    |
| biome-wasm | 1.16       | 24  | 860.53   | 868.89   | 875.14   | 880.88   | 885.92   | 836.43   | 887.19   | 5.01x                 | 5.01x    |

**Files (intersection):** 923

**Throughput:** prettier 0.5 MB/s, tsv 35.0 MB/s, tsv-wasm 24.5 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript   | 0.77       | 21  | 1299.35  | 1305.57  | 1309.32  | 1312.89  | 1325.33  | 1265.81  | 1328.85  | baseline                      | baseline |
| tsv                | 1.66       | 25  | 602.43   | 604.87   | 608.16   | 611.19   | 612.48   | 593.22   | 612.68   | 2.16x                         | 2.16x    |
| tsv-wasm           | 1.63       | 23  | 607.94   | 621.34   | 628.05   | 630.99   | 632.79   | 602.83   | 633.30   | 2.12x                         | 2.14x    |
| tsv+locations      | 1.26       | 24  | 790.76   | 795.65   | 798.07   | 799.98   | 801.27   | 785.71   | 801.57   | 1.64x                         | 1.64x    |
| tsv-wasm+locations | 1.22       | 23  | 820.08   | 825.41   | 830.07   | 830.38   | 831.93   | 804.87   | 832.37   | 1.58x                         | 1.58x    |
| tsv-internal       | 12.43      | 188 | 80.42    | 80.67    | 80.98    | 81.03    | 81.19    | 79.56    | 81.68    | 16.1x                         | 16.2x    |
| tsv-wasm-internal  | 8.11       | 113 | 123.92   | 124.24   | 124.68   | 124.98   | 125.25   | 121.22   | 125.55   | 10.5x                         | 10.5x    |
| oxc-parser         | 1.19       | 20  | 855.51   | 866.24   | 869.58   | 870.64   | 874.77   | 821.41   | 875.95   | 1.54x                         | 1.52x    |
| oxc-parser-wasm    | 0.97       | 23  | 1031.19  | 1054.37  | 1063.94  | 1070.58  | 1078.13  | 987.34   | 1080.21  | 1.26x                         | 1.26x    |
| yuku-parser        | 3.06       | 47  | 325.56   | 332.40   | 333.66   | 334.27   | 335.03   | 319.02   | 335.23   | 3.98x                         | 3.99x    |
| yuku-parser-wasm   | 4.10       | 60  | 244.93   | 249.80   | 251.57   | 252.67   | 263.84   | 234.84   | 280.76   | 5.32x                         | 5.30x    |
| swc                | 0.75       | 24  | 1331.24  | 1333.35  | 1335.87  | 1336.31  | 1337.86  | 1294.57  | 1338.31  | 0.98x                         | 0.98x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 14.2 MB/s, tsv 30.6 MB/s, tsv-wasm 30.1 MB/s, tsv+locations 23.3 MB/s, tsv-wasm+locations 22.5 MB/s, tsv-internal 229.1 MB/s, tsv-wasm-internal 149.4 MB/s, oxc-parser 21.9 MB/s, oxc-parser-wasm 17.9 MB/s, yuku-parser 56.4 MB/s, yuku-parser-wasm 75.5 MB/s, swc 13.9 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.5x tsv-internal, tsv-wasm 5.0x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 24 | 14.53   | 14.70   | 14.87   | 14.90   | 14.98   | 13.71   | 15.01   | baseline              | baseline |
| tsv         | 2.65       | 38 | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 38.2x                 | 38.5x    |
| tsv-wasm    | 1.84       | 30 | 0.54    | 0.55    | 0.55    | 0.55    | 0.55    | 0.54    | 0.55    | 26.6x                 | 26.7x    |
| oxfmt       | 1.19       | 24 | 0.84    | 0.85    | 0.86    | 0.86    | 0.86    | 0.82    | 0.86    | 17.1x                 | 17.3x    |
| biome-wasm  | 0.26       | 21 | 3.87    | 3.89    | 3.91    | 3.91    | 3.91    | 3.85    | 3.91    | 3.72x                 | 3.75x    |
| dprint-wasm | 0.31       | 22 | 3.19    | 3.21    | 3.21    | 3.21    | 3.22    | 3.17    | 3.22    | 4.52x                 | 4.55x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.3 MB/s, tsv 48.8 MB/s, tsv-wasm 33.9 MB/s, oxfmt 21.9 MB/s, biome-wasm 4.8 MB/s, dprint-wasm 5.8 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 51.05      | 765  | 19.64    | 20.11    | 20.53    | 20.72    | 21.17    | 17.55    | 22.92    | baseline                     | baseline |
| tsv                | 67.09      | 1008 | 14.69    | 15.47    | 16.22    | 16.80    | 17.48    | 13.46    | 18.02    | 1.31x                        | 1.34x    |
| tsv-wasm           | 77.72      | 1167 | 12.81    | 13.16    | 13.56    | 13.73    | 14.17    | 12.15    | 14.92    | 1.52x                        | 1.53x    |
| tsv+locations      | 51.54      | 760  | 19.41    | 19.59    | 19.82    | 19.95    | 20.35    | 18.57    | 21.84    | 1.01x                        | 1.01x    |
| tsv-wasm+locations | 59.01      | 882  | 16.90    | 17.27    | 17.61    | 17.87    | 18.39    | 15.92    | 20.98    | 1.16x                        | 1.16x    |
| tsv-internal       | 430.78     | 6005 | 2.32     | 2.33     | 2.34     | 2.36     | 2.42     | 2.29     | 3.34     | 8.44x                        | 8.46x    |
| tsv-wasm-internal  | 302.18     | 4189 | 3.31     | 3.33     | 3.34     | 3.36     | 3.44     | 3.26     | 4.83     | 5.92x                        | 5.93x    |
| postcss            | 59.21      | 880  | 16.88    | 17.41    | 17.87    | 18.09    | 18.53    | 14.17    | 20.19    | 1.16x                        | 1.16x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 20.1 MB/s, tsv 26.4 MB/s, tsv-wasm 30.5 MB/s, tsv+locations 20.3 MB/s, tsv-wasm+locations 23.2 MB/s, tsv-internal 169.3 MB/s, tsv-wasm-internal 118.7 MB/s, postcss 23.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.4x tsv-internal, tsv-wasm 3.9x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.18       | 32   | 458.53   | 462.57   | 471.04   | 471.45   | 473.70   | 449.09   | 474.55   | baseline              | baseline |
| tsv        | 217.94     | 2748 | 4.59     | 4.63     | 4.82     | 5.06     | 5.23     | 4.53     | 5.70     | 100.1x                | 99.9x    |
| tsv-wasm   | 159.27     | 2165 | 6.28     | 6.31     | 6.38     | 6.50     | 6.72     | 6.18     | 10.81    | 73.2x                 | 73.0x    |
| oxfmt      | 55.22      | 821  | 18.12    | 18.46    | 18.74    | 18.97    | 19.74    | 16.63    | 20.41    | 25.4x                 | 25.3x    |
| biome-wasm | 12.42      | 174  | 80.15    | 82.21    | 84.63    | 85.04    | 86.57    | 77.02    | 88.56    | 5.71x                 | 5.72x    |
| malva-wasm | 18.28      | 239  | 55.13    | 55.36    | 55.64    | 55.78    | 56.20    | 53.38    | 56.56    | 8.40x                 | 8.32x    |

**Files (intersection):** 54

**Throughput:** prettier 0.8 MB/s, tsv 77.1 MB/s, tsv-wasm 56.4 MB/s, oxfmt 19.5 MB/s, biome-wasm 4.4 MB/s, malva-wasm 6.5 MB/s

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
| tsv (ffi) | 3.6 MB | 1.7 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.3 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 698.7 KB | 0.4x | 0.4x |
| tsv (napi) | 4.0 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.9 MB | 4.5 MB | 2.8x | 2.5x |
| oxc-parser (napi) | 2.1 MB | 880.8 KB | 0.5x | 0.5x |
| oxfmt (napi) | 8.8 MB | 3.7 MB | 2.2x | 2.0x |
| yuku-parser (napi) | 681.1 KB | 286.0 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 2.3x | 2.0x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 4.5x | 4.1x |
| swc (napi) | 10.5 MB | 10.2 MB | 2.7x | 5.5x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **64.8x** prettier, **66.1x** oxfmt |
| format typescript (2593f) | **38.2x** prettier, **2.23x** oxfmt |
| format css (54f) | **100.1x** prettier, **3.95x** oxfmt |
| parse svelte (929f) | **3.52x** svelte/compiler, **4.77x** rsvelte-parse |
| parse typescript (2593f) | **2.16x** acorn-typescript, **1.40x** oxc-parser, **0.54x** yuku-parser, **2.20x** swc |
| parse css (55f) | **1.31x** svelte/compiler, **1.13x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **45.4x** prettier, **9.07x** biome-wasm |
| format typescript (2593f) | **26.6x** prettier, **7.14x** biome-wasm, **5.88x** dprint-wasm |
| format css (54f) | **73.2x** prettier, **12.8x** biome-wasm, **8.71x** malva-wasm |
| parse svelte (929f) | **3.27x** svelte/compiler |
| parse typescript (2593f) | **2.12x** acorn-typescript, **1.68x** oxc-parser-wasm, **0.40x** yuku-parser-wasm |
| parse css (55f) | **1.52x** svelte/compiler, **1.31x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.43x, wasm 1.40x | typescript: native 1.31x, wasm 1.34x | css: native 1.30x, wasm 1.32x

## Unstable Rows

2 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| format/typescript/prettier | 2.7% | 2.7% | -1.1% | 6.0% | 24/24 |
| parse/typescript/yuku-parser-wasm | 2.4% | 3.0% | -0.4% | 5.7% | 60/63 |

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
