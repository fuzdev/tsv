# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-08T14:22:06.374Z — tsv 0.6.0 (b24d1494)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.4% apart at the median, 1.8% at the 95th percentile and 3.6% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.84       | 44  | 352.56   | 355.79   | 358.58   | 360.43   | 364.21   | 344.45   | 365.30   | baseline                     | baseline |
| tsv                         | 6.87       | 103 | 145.47   | 146.07   | 146.38   | 146.68   | 149.86   | 144.60   | 150.55   | 2.42x                        | 2.42x    |
| tsv-wasm                    | 6.21       | 95  | 160.99   | 161.46   | 162.38   | 162.58   | 163.15   | 159.17   | 163.17   | 2.19x                        | 2.19x    |
| tsv+locations               | 5.26       | 76  | 190.04   | 190.97   | 191.95   | 192.95   | 205.63   | 187.99   | 207.79   | 1.85x                        | 1.86x    |
| tsv-wasm+locations          | 4.85       | 71  | 205.92   | 207.93   | 208.66   | 209.40   | 222.32   | 203.92   | 223.37   | 1.71x                        | 1.71x    |
| tsv-internal                | 60.48      | 871 | 16.53    | 16.59    | 16.65    | 16.74    | 17.07    | 16.38    | 17.25    | 21.3x                        | 21.3x    |
| tsv-wasm-internal           | 42.01      | 571 | 23.75    | 23.99    | 24.10    | 24.25    | 24.41    | 23.54    | 24.64    | 14.8x                        | 14.8x    |
| rsvelte-parse               | 1.71       | 26  | 585.77   | 586.71   | 588.06   | 588.45   | 588.51   | 583.61   | 588.52   | 0.60x                        | 0.60x    |
| rsvelte-parse-skip-expr-loc | 2.61       | 42  | 383.29   | 384.59   | 385.07   | 385.73   | 386.34   | 380.08   | 386.42   | 0.92x                        | 0.92x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 6.8 MB/s, tsv 16.6 MB/s, tsv-wasm 15.0 MB/s, tsv+locations 12.7 MB/s, tsv-wasm+locations 11.7 MB/s, tsv-internal 145.9 MB/s, tsv-wasm-internal 101.3 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.8x tsv-internal, tsv-wasm 6.8x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.19       | 24  | 5397.00  | 5424.33  | 5442.98  | 5457.23  | 5459.66  | 5337.32  | 5459.77  | baseline              | baseline |
| tsv        | 14.76      | 193 | 67.76    | 67.91    | 68.33    | 68.51    | 68.83    | 67.45    | 69.20    | 79.7x                 | 79.6x    |
| tsv-wasm   | 10.25      | 153 | 97.45    | 97.74    | 98.25    | 98.46    | 98.68    | 96.77    | 99.48    | 55.4x                 | 55.4x    |
| oxfmt      | 0.19       | 23  | 5342.40  | 5362.18  | 5385.30  | 5407.78  | 5436.55  | 5296.28  | 5444.16  | 1.01x                 | 1.01x    |
| biome-wasm | 1.01       | 21  | 991.59   | 1000.79  | 1068.57  | 1083.61  | 1084.28  | 980.23   | 1084.46  | 5.43x                 | 5.44x    |

**Files (intersection):** 923

**Throughput:** prettier 0.4 MB/s, tsv 34.4 MB/s, tsv-wasm 23.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.3 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.71       | 24  | 1.40    | 1.42    | 1.43    | 1.43    | 1.43    | 1.37    | 1.43    | baseline                      | baseline |
| tsv                | 1.07       | 21  | 0.93    | 0.93    | 0.94    | 0.95    | 0.95    | 0.92    | 0.95    | 1.51x                         | 1.51x    |
| tsv-wasm           | 1.01       | 20  | 0.99    | 0.99    | 1.01    | 1.02    | 1.02    | 0.99    | 1.02    | 1.42x                         | 1.42x    |
| tsv+locations      | 0.89       | 23  | 1.12    | 1.14    | 1.14    | 1.15    | 1.15    | 1.12    | 1.15    | 1.24x                         | 1.25x    |
| tsv-wasm+locations | 0.84       | 23  | 1.19    | 1.20    | 1.21    | 1.21    | 1.21    | 1.18    | 1.21    | 1.18x                         | 1.18x    |
| tsv-internal       | 9.87       | 143 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 13.8x                         | 13.9x    |
| tsv-wasm-internal  | 7.41       | 110 | 0.13    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 10.4x                         | 10.4x    |
| oxc-parser         | 0.73       | 24  | 1.37    | 1.38    | 1.39    | 1.39    | 1.39    | 1.36    | 1.39    | 1.02x                         | 1.02x    |
| oxc-parser-wasm    | 0.70       | 24  | 1.44    | 1.44    | 1.44    | 1.45    | 1.45    | 1.43    | 1.45    | 0.98x                         | 0.98x    |
| yuku-parser        | 2.60       | 41  | 0.38    | 0.39    | 0.40    | 0.40    | 0.40    | 0.37    | 0.40    | 3.65x                         | 3.67x    |
| yuku-parser-wasm   | 3.38       | 52  | 0.30    | 0.30    | 0.30    | 0.30    | 0.30    | 0.29    | 0.30    | 4.74x                         | 4.75x    |
| swc                | 0.57       | 24  | 1.76    | 1.77    | 1.77    | 1.77    | 1.77    | 1.75    | 1.77    | 0.80x                         | 0.80x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 13.1 MB/s, tsv 19.8 MB/s, tsv-wasm 18.6 MB/s, tsv+locations 16.3 MB/s, tsv-wasm+locations 15.5 MB/s, tsv-internal 181.9 MB/s, tsv-wasm-internal 136.5 MB/s, oxc-parser 13.4 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 48.0 MB/s, yuku-parser-wasm 62.3 MB/s, swc 10.5 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.2x tsv-internal, tsv-wasm 7.3x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 21 | 14.80   | 14.94   | 14.96   | 14.99   | 15.04   | 14.62   | 15.05   | baseline              | baseline |
| tsv         | 2.52       | 38 | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 37.2x                 | 37.2x    |
| tsv-wasm    | 1.81       | 27 | 0.55    | 0.56    | 0.56    | 0.56    | 0.56    | 0.55    | 0.56    | 26.8x                 | 26.8x    |
| oxfmt       | 1.17       | 22 | 0.85    | 0.86    | 0.87    | 0.88    | 0.88    | 0.85    | 0.88    | 17.3x                 | 17.3x    |
| biome-wasm  | 0.24       | 20 | 4.25    | 4.28    | 4.38    | 4.42    | 4.46    | 4.22    | 4.47    | 3.48x                 | 3.48x    |
| dprint-wasm | 0.30       | 24 | 3.30    | 3.30    | 3.31    | 3.31    | 3.31    | 3.28    | 3.31    | 4.49x                 | 4.49x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.2 MB/s, tsv 46.4 MB/s, tsv-wasm 33.4 MB/s, oxfmt 21.5 MB/s, biome-wasm 4.3 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 95.45      | 1233 | 10.49    | 10.69    | 11.05    | 11.24    | 11.58    | 10.22    | 12.25    | baseline                     | baseline |
| tsv                | 50.92      | 756  | 19.52    | 20.04    | 20.17    | 20.27    | 20.62    | 18.58    | 21.52    | 0.53x                        | 0.54x    |
| tsv-wasm           | 48.56      | 580  | 20.69    | 20.86    | 21.59    | 23.60    | 24.80    | 19.60    | 25.29    | 0.51x                        | 0.51x    |
| tsv+locations      | 43.16      | 622  | 23.25    | 23.38    | 23.49    | 23.62    | 24.09    | 22.10    | 26.62    | 0.45x                        | 0.45x    |
| tsv-wasm+locations | 42.60      | 485  | 23.51    | 23.84    | 25.52    | 25.96    | 26.72    | 23.19    | 27.95    | 0.45x                        | 0.45x    |
| tsv-internal       | 360.35     | 4980 | 2.78     | 2.78     | 2.79     | 2.81     | 2.89     | 2.75     | 3.53     | 3.78x                        | 3.78x    |
| tsv-wasm-internal  | 245.44     | 3342 | 4.07     | 4.08     | 4.12     | 4.14     | 4.23     | 4.03     | 8.09     | 2.57x                        | 2.57x    |
| postcss            | 83.61      | 976  | 11.97    | 12.23    | 12.76    | 12.99    | 13.75    | 11.76    | 14.06    | 0.88x                        | 0.88x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 37.5 MB/s, tsv 20.0 MB/s, tsv-wasm 19.1 MB/s, tsv+locations 17.0 MB/s, tsv-wasm+locations 16.7 MB/s, tsv-internal 141.6 MB/s, tsv-wasm-internal 96.4 MB/s, postcss 32.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.1x tsv-internal, tsv-wasm 5.1x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.76       | 25   | 566.63   | 575.08   | 579.21   | 582.00   | 583.94   | 558.76   | 584.38   | baseline              | baseline |
| tsv        | 193.42     | 2587 | 5.17     | 5.19     | 5.26     | 5.53     | 5.62     | 5.10     | 6.00     | 109.7x                | 109.6x   |
| tsv-wasm   | 136.35     | 1769 | 7.34     | 7.38     | 7.48     | 7.68     | 7.83     | 7.22     | 9.28     | 77.3x                 | 77.2x    |
| oxfmt      | 54.52      | 805  | 18.34    | 18.59    | 18.88    | 19.06    | 19.59    | 17.10    | 20.84    | 30.9x                 | 30.9x    |
| biome-wasm | 11.57      | 120  | 86.68    | 92.07    | 94.06    | 95.12    | 98.78    | 85.02    | 100.74   | 6.56x                 | 6.54x    |
| malva-wasm | 21.26      | 267  | 47.09    | 47.23    | 47.56    | 47.71    | 47.96    | 46.72    | 51.74    | 12.1x                 | 12.0x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 68.5 MB/s, tsv-wasm 48.3 MB/s, oxfmt 19.3 MB/s, biome-wasm 4.1 MB/s, malva-wasm 7.5 MB/s

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
| format svelte (923f) | **79.7x** prettier, **78.9x** oxfmt |
| format typescript (2593f) | **37.2x** prettier, **2.16x** oxfmt |
| format css (54f) | **109.7x** prettier, **3.55x** oxfmt |
| parse svelte (929f) | **2.42x** svelte/compiler, **4.02x** rsvelte-parse |
| parse typescript (2593f) | **1.51x** acorn-typescript, **1.48x** oxc-parser, **0.41x** yuku-parser, **1.89x** swc |
| parse css (55f) | **0.53x** svelte/compiler, **0.61x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **55.4x** prettier, **10.2x** biome-wasm |
| format typescript (2593f) | **26.8x** prettier, **7.70x** biome-wasm, **5.97x** dprint-wasm |
| format css (54f) | **77.3x** prettier, **11.8x** biome-wasm, **6.41x** malva-wasm |
| parse svelte (929f) | **2.19x** svelte/compiler |
| parse typescript (2593f) | **1.42x** acorn-typescript, **1.45x** oxc-parser-wasm, **0.30x** yuku-parser-wasm |
| parse css (55f) | **0.51x** svelte/compiler, **0.58x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.31x, wasm 1.28x | typescript: native 1.21x, wasm 1.20x | css: native 1.18x, wasm 1.14x

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
