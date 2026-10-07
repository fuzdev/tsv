# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.7

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-07T01:52:32.975Z — tsv 0.6.0 (3a8e53d7)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.25       | 14  | 444.43   | 448.35   | 457.77   | 468.63   | 473.42   | 438.37   | 474.61   | baseline                     | baseline |
| tsv                         | 7.15       | 35  | 139.98   | 140.19   | 140.65   | 140.78   | 143.33   | 139.16   | 144.58   | 3.17x                        | 3.17x    |
| tsv-wasm                    | 5.89       | 29  | 169.91   | 170.26   | 170.45   | 170.93   | 173.47   | 169.09   | 174.50   | 2.61x                        | 2.62x    |
| tsv+locations               | 5.00       | 26  | 199.78   | 200.32   | 200.68   | 200.95   | 201.02   | 198.76   | 201.04   | 2.22x                        | 2.22x    |
| tsv-wasm+locations          | 4.37       | 22  | 229.08   | 229.56   | 229.92   | 230.25   | 230.36   | 228.15   | 230.38   | 1.94x                        | 1.94x    |
| tsv-internal                | 62.37      | 288 | 16.03    | 16.06    | 16.12    | 16.24    | 16.50    | 15.93    | 16.84    | 27.7x                        | 27.7x    |
| tsv-wasm-internal           | 35.96      | 166 | 27.81    | 27.84    | 27.91    | 28.11    | 28.41    | 27.69    | 28.47    | 16.0x                        | 16.0x    |
| rsvelte-parse               | 1.75       | 9   | 572.27   | 573.14   | 574.02   | —        | —        | 570.24   | 574.49   | 0.77x                        | 0.78x    |
| rsvelte-parse-skip-expr-loc | 2.69       | 14  | 372.86   | 373.25   | 373.65   | 373.73   | 373.84   | 369.73   | 373.87   | 1.19x                        | 1.19x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 5.4 MB/s, tsv 17.2 MB/s, tsv-wasm 14.2 MB/s, tsv+locations 12.1 MB/s, tsv-wasm+locations 10.5 MB/s, tsv-internal 150.4 MB/s, tsv-wasm-internal 86.7 MB/s, rsvelte-parse 4.2 MB/s, rsvelte-parse-skip-expr-loc 6.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.7x tsv-internal, tsv-wasm 6.1x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.20       | 16 | 5007.83  | 5074.95  | 5113.12  | 5128.96  | 5141.48  | 4877.06  | 5144.61  | baseline              | baseline |
| tsv        | 14.86      | 67 | 67.23    | 67.59    | 67.91    | 68.16    | 68.62    | 67.04    | 69.75    | 74.6x                 | 74.5x    |
| tsv-wasm   | 9.30       | 42 | 107.38   | 108.21   | 108.60   | 111.49   | 112.81   | 107.07   | 112.83   | 46.7x                 | 46.6x    |
| oxfmt      | 0.20       | 8  | 5073.19  | 5103.19  | 5112.53  | —        | —        | 5023.97  | 5131.00  | 0.99x                 | 0.99x    |
| biome-wasm | 1.08       | 8  | 922.24   | 933.92   | 941.43   | —        | —        | 913.91   | 942.30   | 5.42x                 | 5.43x    |

**Files (intersection):** 923

**Throughput:** prettier 0.5 MB/s, tsv 34.6 MB/s, tsv-wasm 21.7 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.38       | 14 | 2.64    | 2.65    | 2.67    | 2.69    | 2.69    | 2.63    | 2.69    | baseline                      | baseline |
| tsv                | 1.15       | 7  | 0.87    | 0.87    | 0.87    | —       | —       | 0.87    | 0.88    | 3.02x                         | 3.02x    |
| tsv-wasm           | 0.97       | 8  | 1.03    | 1.03    | 1.03    | —       | —       | 1.03    | 1.03    | 2.56x                         | 2.56x    |
| tsv+locations      | 0.90       | 7  | 1.11    | 1.11    | 1.12    | —       | —       | 1.11    | 1.12    | 2.37x                         | 2.37x    |
| tsv-wasm+locations | 0.79       | 8  | 1.27    | 1.27    | 1.27    | —       | —       | 1.26    | 1.27    | 2.08x                         | 2.08x    |
| tsv-internal       | 11.02      | 56 | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 29.1x                         | 29.1x    |
| tsv-wasm-internal  | 6.41       | 32 | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 16.9x                         | 16.9x    |
| oxc-parser         | 0.79       | 8  | 1.27    | 1.27    | 1.28    | —       | —       | 1.23    | 1.29    | 2.10x                         | 2.08x    |
| oxc-parser-wasm    | 0.72       | 8  | 1.39    | 1.39    | 1.40    | —       | —       | 1.38    | 1.40    | 1.90x                         | 1.90x    |
| yuku-parser        | 2.24       | 10 | 0.45    | 0.45    | 0.46    | 0.53    | 0.59    | 0.43    | 0.61    | 5.91x                         | 5.85x    |
| yuku-parser-wasm   | 2.61       | 12 | 0.39    | 0.39    | 0.39    | 0.43    | 0.47    | 0.37    | 0.48    | 6.90x                         | 6.76x    |
| swc                | 0.58       | 7  | 1.71    | 1.71    | 1.72    | —       | —       | 1.71    | 1.72    | 1.54x                         | 1.54x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 7.0 MB/s, tsv 21.1 MB/s, tsv-wasm 17.9 MB/s, tsv+locations 16.5 MB/s, tsv-wasm+locations 14.6 MB/s, tsv-internal 203.0 MB/s, tsv-wasm-internal 118.0 MB/s, oxc-parser 14.6 MB/s, oxc-parser-wasm 13.3 MB/s, yuku-parser 41.3 MB/s, yuku-parser-wasm 48.2 MB/s, swc 10.8 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.6x tsv-internal, tsv-wasm 6.6x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.08       | 16 | 13.14   | 13.21   | 13.25   | 13.29   | 13.35   | 12.97   | 13.36   | baseline              | baseline |
| tsv         | 2.65       | 14 | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 34.9x                 | 34.9x    |
| tsv-wasm    | 1.61       | 8  | 0.62    | 0.62    | 0.62    | —       | —       | 0.62    | 0.63    | 21.1x                 | 21.1x    |
| oxfmt       | 1.17       | 6  | 0.86    | 0.86    | 0.86    | —       | —       | 0.85    | 0.86    | 15.3x                 | 15.3x    |
| biome-wasm  | 0.23       | 8  | 4.43    | 4.43    | 4.44    | —       | —       | 4.42    | 4.44    | 2.97x                 | 2.96x    |
| dprint-wasm | 0.27       | 8  | 3.72    | 3.72    | 3.73    | —       | —       | 3.72    | 3.73    | 3.53x                 | 3.53x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.4 MB/s, tsv 48.9 MB/s, tsv-wasm 29.7 MB/s, oxfmt 21.5 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 77.87      | 390  | 12.77    | 13.04    | 13.33    | 13.39    | 13.64    | 12.40    | 13.99    | baseline                     | baseline |
| tsv                | 54.88      | 254  | 18.23    | 18.40    | 18.79    | 19.63    | 20.78    | 17.73    | 32.60    | 0.70x                        | 0.70x    |
| tsv-wasm           | 45.94      | 202  | 21.75    | 22.06    | 23.11    | 23.81    | 25.01    | 21.27    | 32.14    | 0.59x                        | 0.59x    |
| tsv+locations      | 42.95      | 186  | 23.29    | 23.57    | 24.46    | 25.07    | 25.52    | 22.94    | 26.06    | 0.55x                        | 0.55x    |
| tsv-wasm+locations | 37.31      | 163  | 26.78    | 27.21    | 28.22    | 28.43    | 28.90    | 26.26    | 29.32    | 0.48x                        | 0.48x    |
| tsv-internal       | 399.90     | 1842 | 2.50     | 2.51     | 2.52     | 2.53     | 2.61     | 2.48     | 3.08     | 5.14x                        | 5.11x    |
| tsv-wasm-internal  | 215.72     | 1066 | 4.64     | 4.66     | 4.69     | 4.70     | 4.81     | 4.58     | 5.08     | 2.77x                        | 2.76x    |
| postcss            | 80.73      | 402  | 12.33    | 12.64    | 12.83    | 12.98    | 13.19    | 11.94    | 13.73    | 1.04x                        | 1.04x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 30.6 MB/s, tsv 21.6 MB/s, tsv-wasm 18.1 MB/s, tsv+locations 16.9 MB/s, tsv-wasm+locations 14.7 MB/s, tsv-internal 157.1 MB/s, tsv-wasm-internal 84.8 MB/s, postcss 31.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.3x tsv-internal, tsv-wasm 4.7x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.82       | 15  | 548.59   | 552.16   | 562.14   | 572.10   | 578.01   | 539.00   | 579.48   | baseline              | baseline |
| tsv        | 217.06     | 976 | 4.61     | 4.62     | 4.65     | 4.68     | 4.92     | 4.57     | 8.63     | 119.0x                | 119.1x   |
| tsv-wasm   | 120.50     | 493 | 8.30     | 8.33     | 8.41     | 8.45     | 8.66     | 8.26     | 9.14     | 66.1x                 | 66.1x    |
| oxfmt      | 54.17      | 258 | 18.46    | 18.74    | 19.06    | 19.32    | 19.97    | 17.33    | 20.22    | 29.7x                 | 29.7x    |
| biome-wasm | 10.61      | 49  | 94.13    | 95.25    | 95.79    | 96.92    | 110.92   | 92.98    | 111.60   | 5.82x                 | 5.83x    |
| malva-wasm | 19.44      | 90  | 51.43    | 51.55    | 51.90    | 52.28    | 52.58    | 51.19    | 52.98    | 10.7x                 | 10.7x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 76.8 MB/s, tsv-wasm 42.7 MB/s, oxfmt 19.2 MB/s, biome-wasm 3.8 MB/s, malva-wasm 6.9 MB/s

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
| format svelte (923f) | **74.6x** prettier, **75.4x** oxfmt |
| format typescript (2593f) | **34.9x** prettier, **2.28x** oxfmt |
| format css (54f) | **119.0x** prettier, **4.01x** oxfmt |
| parse svelte (929f) | **3.17x** svelte/compiler, **4.09x** rsvelte-parse |
| parse typescript (2593f) | **3.02x** acorn-typescript, **1.44x** oxc-parser, **0.51x** yuku-parser, **1.96x** swc |
| parse css (55f) | **0.70x** svelte/compiler, **0.68x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **46.7x** prettier, **8.61x** biome-wasm |
| format typescript (2593f) | **21.1x** prettier, **7.13x** biome-wasm, **5.99x** dprint-wasm |
| format css (54f) | **66.1x** prettier, **11.4x** biome-wasm, **6.20x** malva-wasm |
| parse svelte (929f) | **2.61x** svelte/compiler |
| parse typescript (2593f) | **2.56x** acorn-typescript, **1.35x** oxc-parser-wasm, **0.37x** yuku-parser-wasm |
| parse css (55f) | **0.59x** svelte/compiler, **0.57x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.43x, wasm 1.35x | typescript: native 1.28x, wasm 1.23x | css: native 1.28x, wasm 1.23x

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/typescript/yuku-parser | 1.9% | 10.8% | -0.2% | 10/11 |

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
