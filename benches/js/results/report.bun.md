# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-10T18:20:23.079Z — tsv 0.6.0 (c93c1649)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.5, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.6% apart at the median, 3.0% at the 95th percentile and 5.7% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.73       | 40  | 367.09   | 372.23   | 377.34   | 380.32   | 384.59   | 355.43   | 385.84   | baseline                     | baseline |
| tsv                         | 9.58       | 144 | 104.11   | 105.31   | 106.48   | 106.88   | 108.28   | 102.46   | 110.40   | 3.51x                        | 3.53x    |
| tsv-wasm                    | 8.92       | 128 | 112.15   | 112.76   | 113.89   | 114.43   | 115.29   | 110.35   | 115.74   | 3.27x                        | 3.27x    |
| tsv+locations               | 6.76       | 102 | 147.85   | 149.28   | 150.63   | 151.25   | 152.29   | 144.60   | 154.00   | 2.48x                        | 2.48x    |
| tsv-wasm+locations          | 6.30       | 96  | 158.69   | 159.66   | 160.52   | 160.83   | 162.09   | 155.65   | 164.67   | 2.31x                        | 2.31x    |
| tsv-internal                | 69.62      | 930 | 14.38    | 14.42    | 14.54    | 14.69    | 14.87    | 14.22    | 15.08    | 25.5x                        | 25.5x    |
| tsv-wasm-internal           | 43.20      | 573 | 23.15    | 23.21    | 23.44    | 23.64    | 24.03    | 23.01    | 24.82    | 15.8x                        | 15.9x    |
| rsvelte-parse               | 2.00       | 31  | 500.31   | 501.59   | 502.50   | 503.40   | 504.43   | 496.63   | 504.81   | 0.73x                        | 0.73x    |
| rsvelte-parse-skip-expr-loc | 2.98       | 45  | 335.75   | 336.95   | 338.27   | 338.77   | 339.00   | 331.02   | 339.11   | 1.09x                        | 1.09x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 6.6 MB/s, tsv 23.1 MB/s, tsv-wasm 21.5 MB/s, tsv+locations 16.3 MB/s, tsv-wasm+locations 15.2 MB/s, tsv-internal 167.9 MB/s, tsv-wasm-internal 104.2 MB/s, rsvelte-parse 4.8 MB/s, rsvelte-parse-skip-expr-loc 7.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.3x tsv-internal, tsv-wasm 4.8x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.23       | 22  | 4257.82  | 4365.66  | 4454.62  | 4528.19  | 4552.80  | 4088.50  | 4556.82  | baseline              | baseline |
| tsv        | 15.04      | 215 | 66.48    | 66.74    | 67.15    | 67.60    | 67.97    | 65.57    | 68.10    | 64.3x                 | 64.0x    |
| tsv-wasm   | 10.55      | 160 | 94.73    | 95.24    | 95.76    | 95.99    | 96.41    | 93.44    | 96.90    | 45.1x                 | 44.9x    |
| oxfmt      | 0.23       | 24  | 4405.08  | 4459.01  | 4475.36  | 4487.74  | 4525.55  | 4301.63  | 4536.40  | 0.97x                 | 0.97x    |
| biome-wasm | 1.16       | 24  | 858.66   | 870.77   | 876.69   | 877.05   | 877.50   | 839.68   | 877.63   | 4.97x                 | 4.96x    |

**Files (intersection):** 923

**Throughput:** prettier 0.5 MB/s, tsv 35.0 MB/s, tsv-wasm 24.6 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 868/929 (93%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript   | 0.77       | 24  | 1298.30  | 1316.52  | 1329.37  | 1330.77  | 1333.00  | 1257.28  | 1333.63  | baseline                      | baseline |
| tsv                | 1.68       | 26  | 600.79   | 604.13   | 608.04   | 608.39   | 609.12   | 581.60   | 609.34   | 2.18x                         | 2.16x    |
| tsv-wasm           | 1.60       | 27  | 622.70   | 624.60   | 625.87   | 627.05   | 628.82   | 618.34   | 629.30   | 2.09x                         | 2.08x    |
| tsv+locations      | 1.26       | 22  | 794.96   | 798.53   | 802.59   | 804.30   | 806.08   | 781.92   | 806.59   | 1.64x                         | 1.63x    |
| tsv-wasm+locations | 1.21       | 24  | 822.76   | 833.59   | 839.83   | 843.20   | 848.98   | 803.42   | 850.59   | 1.58x                         | 1.58x    |
| tsv-internal       | 12.37      | 182 | 80.85    | 81.00    | 81.30    | 81.41    | 81.66    | 80.00    | 81.82    | 16.1x                         | 16.1x    |
| tsv-wasm-internal  | 8.07       | 113 | 123.64   | 124.76   | 125.08   | 125.54   | 125.93   | 122.57   | 126.04   | 10.5x                         | 10.5x    |
| oxc-parser         | 1.17       | 24  | 854.87   | 862.31   | 866.51   | 869.71   | 871.77   | 833.76   | 872.25   | 1.52x                         | 1.52x    |
| oxc-parser-wasm    | 0.97       | 24  | 1030.24  | 1052.41  | 1070.30  | 1071.89  | 1076.08  | 978.72   | 1077.31  | 1.26x                         | 1.26x    |
| yuku-parser        | 3.06       | 48  | 326.04   | 328.96   | 330.51   | 331.92   | 332.72   | 321.79   | 333.30   | 3.98x                         | 3.98x    |
| yuku-parser-wasm   | 4.08       | 63  | 244.87   | 247.29   | 249.32   | 249.92   | 251.34   | 239.38   | 252.04   | 5.30x                         | 5.30x    |
| swc                | 0.75       | 23  | 1342.25  | 1354.87  | 1358.03  | 1358.40  | 1361.05  | 1293.86  | 1361.84  | 0.97x                         | 0.97x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 14.2 MB/s, tsv 30.9 MB/s, tsv-wasm 29.6 MB/s, tsv+locations 23.2 MB/s, tsv-wasm+locations 22.4 MB/s, tsv-internal 227.9 MB/s, tsv-wasm-internal 148.7 MB/s, oxc-parser 21.6 MB/s, oxc-parser-wasm 17.9 MB/s, yuku-parser 56.4 MB/s, yuku-parser-wasm 75.1 MB/s, swc 13.8 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.4x tsv-internal, tsv-wasm 5.0x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 24 | 14.08   | 14.66   | 14.74   | 14.76   | 14.77   | 13.65   | 14.77   | baseline              | baseline |
| tsv         | 2.65       | 42 | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 37.7x                 | 37.3x    |
| tsv-wasm    | 1.84       | 29 | 0.54    | 0.55    | 0.55    | 0.55    | 0.55    | 0.54    | 0.55    | 26.2x                 | 25.9x    |
| oxfmt       | 1.20       | 24 | 0.83    | 0.83    | 0.84    | 0.84    | 0.84    | 0.82    | 0.84    | 17.1x                 | 17.0x    |
| biome-wasm  | 0.26       | 21 | 3.87    | 3.89    | 3.90    | 3.90    | 3.90    | 3.85    | 3.90    | 3.67x                 | 3.64x    |
| dprint-wasm | 0.31       | 23 | 3.21    | 3.21    | 3.22    | 3.23    | 3.23    | 3.18    | 3.23    | 4.44x                 | 4.39x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.3 MB/s, tsv 48.8 MB/s, tsv-wasm 33.9 MB/s, oxfmt 22.2 MB/s, biome-wasm 4.8 MB/s, dprint-wasm 5.8 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 50.55      | 757  | 19.80    | 20.27    | 20.70    | 20.90    | 21.53    | 17.77    | 22.70    | baseline                     | baseline |
| tsv                | 67.89      | 1020 | 14.42    | 15.45    | 15.72    | 16.16    | 17.22    | 13.47    | 18.13    | 1.34x                        | 1.37x    |
| tsv-wasm           | 77.92      | 1162 | 12.73    | 13.09    | 13.56    | 13.84    | 14.26    | 12.10    | 14.76    | 1.54x                        | 1.56x    |
| tsv+locations      | 50.92      | 662  | 19.60    | 20.11    | 21.28    | 21.66    | 22.11    | 18.65    | 22.63    | 1.01x                        | 1.01x    |
| tsv-wasm+locations | 58.69      | 870  | 16.99    | 17.35    | 17.69    | 18.03    | 18.78    | 16.16    | 22.03    | 1.16x                        | 1.17x    |
| tsv-internal       | 430.33     | 5968 | 2.33     | 2.33     | 2.34     | 2.36     | 2.44     | 2.30     | 3.25     | 8.51x                        | 8.52x    |
| tsv-wasm-internal  | 298.93     | 4202 | 3.34     | 3.37     | 3.39     | 3.40     | 3.51     | 3.29     | 4.99     | 5.91x                        | 5.92x    |
| postcss            | 58.64      | 877  | 17.03    | 17.59    | 18.05    | 18.25    | 18.74    | 14.35    | 23.59    | 1.16x                        | 1.16x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.9 MB/s, tsv 26.7 MB/s, tsv-wasm 30.6 MB/s, tsv+locations 20.0 MB/s, tsv-wasm+locations 23.1 MB/s, tsv-internal 169.1 MB/s, tsv-wasm-internal 117.5 MB/s, postcss 23.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.3x tsv-internal, tsv-wasm 3.8x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.14       | 33   | 465.86   | 472.65   | 476.49   | 481.57   | 488.44   | 454.86   | 491.60   | baseline              | baseline |
| tsv        | 216.76     | 2694 | 4.61     | 4.66     | 4.84     | 4.97     | 5.22     | 4.54     | 8.67     | 101.3x                | 101.0x   |
| tsv-wasm   | 159.71     | 2167 | 6.26     | 6.29     | 6.34     | 6.47     | 6.67     | 6.20     | 7.48     | 74.7x                 | 74.4x    |
| oxfmt      | 55.66      | 813  | 17.94    | 18.29    | 18.61    | 18.85    | 19.59    | 16.59    | 20.18    | 26.0x                 | 26.0x    |
| biome-wasm | 12.40      | 178  | 79.75    | 82.18    | 84.35    | 85.25    | 86.57    | 77.44    | 86.71    | 5.80x                 | 5.84x    |
| malva-wasm | 17.85      | 244  | 55.98    | 56.26    | 56.57    | 56.76    | 57.15    | 55.67    | 58.88    | 8.34x                 | 8.32x    |

**Files (intersection):** 54

**Throughput:** prettier 0.8 MB/s, tsv 76.7 MB/s, tsv-wasm 56.5 MB/s, oxfmt 19.7 MB/s, biome-wasm 4.4 MB/s, malva-wasm 6.3 MB/s

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
| markup_fmt (wasm) | 2.0 MB | 557.3 KB | 0.7x | 0.5x |
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
| format svelte (923f) | **64.3x** prettier, **66.2x** oxfmt |
| format typescript (2593f) | **37.7x** prettier, **2.20x** oxfmt |
| format css (54f) | **101.3x** prettier, **3.89x** oxfmt |
| parse svelte (929f) | **3.51x** svelte/compiler, **4.79x** rsvelte-parse |
| parse typescript (2593f) | **2.18x** acorn-typescript, **1.43x** oxc-parser, **0.55x** yuku-parser, **2.24x** swc |
| parse css (55f) | **1.34x** svelte/compiler, **1.16x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **45.1x** prettier, **9.07x** biome-wasm |
| format typescript (2593f) | **26.2x** prettier, **7.14x** biome-wasm, **5.90x** dprint-wasm |
| format css (54f) | **74.7x** prettier, **12.9x** biome-wasm, **8.95x** malva-wasm |
| parse svelte (929f) | **3.27x** svelte/compiler |
| parse typescript (2593f) | **2.09x** acorn-typescript, **1.65x** oxc-parser-wasm, **0.39x** yuku-parser-wasm |
| parse css (55f) | **1.54x** svelte/compiler, **1.33x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.42x, wasm 1.42x | typescript: native 1.33x, wasm 1.32x | css: native 1.33x, wasm 1.33x

## Unstable Rows

3 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| format/typescript/prettier | 2.6% | 2.6% | -1.8% | 5.7% | 24/24 |
| format/svelte/prettier | 2.1% | 2.7% | -5.5% | 0.7% | 22/24 |
| parse/typescript/oxc-parser-wasm | 3.0% | 3.0% | +5.0% | 3.0% | 24/24 |

## Skipped Files

71 files skipped, 83 unique file+error combinations — Svelte 66, TypeScript 4, CSS 1 files.

**Per-benchmark skip counts:**
- format/svelte: markup-fmt-wasm: 61
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
