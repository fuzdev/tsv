# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-07T02:28:22.396Z — tsv 0.6.0 (3a8e53d7)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.03       | 16  | 489.65   | 499.23   | 505.64   | 507.22   | 508.08   | 482.53   | 508.29   | baseline                     | baseline |
| tsv                         | 9.44       | 46  | 105.81   | 106.30   | 107.29   | 107.88   | 109.28   | 104.95   | 109.84   | 4.66x                        | 4.63x    |
| tsv-wasm                    | 8.68       | 44  | 115.08   | 116.32   | 117.12   | 118.32   | 119.75   | 112.86   | 120.15   | 4.28x                        | 4.25x    |
| tsv+locations               | 6.70       | 31  | 149.07   | 150.08   | 152.60   | 154.16   | 156.35   | 147.12   | 157.12   | 3.30x                        | 3.28x    |
| tsv-wasm+locations          | 6.17       | 30  | 162.22   | 162.74   | 163.41   | 164.69   | 167.10   | 158.11   | 167.75   | 3.04x                        | 3.02x    |
| tsv-internal                | 68.32      | 310 | 14.64    | 14.67    | 14.75    | 14.91    | 15.15    | 14.56    | 15.20    | 33.7x                        | 33.5x    |
| tsv-wasm-internal           | 42.98      | 192 | 23.27    | 23.30    | 23.47    | 23.64    | 24.07    | 23.17    | 24.41    | 21.2x                        | 21.0x    |
| rsvelte-parse               | 2.00       | 9   | 500.38   | 501.12   | 511.61   | —        | —        | 494.42   | 513.66   | 0.99x                        | 0.98x    |
| rsvelte-parse-skip-expr-loc | 2.95       | 15  | 336.92   | 342.69   | 345.89   | 346.59   | 347.23   | 332.42   | 347.39   | 1.46x                        | 1.45x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 4.9 MB/s, tsv 22.8 MB/s, tsv-wasm 20.9 MB/s, tsv+locations 16.2 MB/s, tsv-wasm+locations 14.9 MB/s, tsv-internal 164.8 MB/s, tsv-wasm-internal 103.6 MB/s, rsvelte-parse 4.8 MB/s, rsvelte-parse-skip-expr-loc 7.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.2x tsv-internal, tsv-wasm 5.0x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.23       | 14 | 4.29    | 4.34    | 4.47    | 4.56    | 4.59    | 4.21    | 4.60    | baseline              | baseline |
| tsv        | 14.53      | 67 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 62.2x                 | 62.4x    |
| tsv-wasm   | 10.29      | 49 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 44.1x                 | 44.2x    |
| oxfmt      | 0.22       | 8  | 4.53    | 4.56    | 4.62    | —       | —       | 4.41    | 4.76    | 0.95x                 | 0.95x    |
| biome-wasm | 0.93       | 8  | 1.09    | 1.09    | 1.10    | —       | —       | 1.05    | 1.11    | 3.97x                 | 3.95x    |

**Files (intersection):** 923

**Throughput:** prettier 0.5 MB/s, tsv 33.8 MB/s, tsv-wasm 24.0 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript   | 0.31       | 11 | 3196.80  | 3201.53  | 3223.20  | 3252.92  | 3288.31  | 3143.94  | 3297.15  | baseline                      | baseline |
| tsv                | 1.64       | 9  | 609.77   | 614.28   | 616.06   | —        | —        | 607.31   | 618.08   | 5.23x                         | 5.24x    |
| tsv-wasm           | 1.58       | 8  | 631.97   | 636.23   | 639.74   | —        | —        | 625.18   | 639.84   | 5.05x                         | 5.06x    |
| tsv+locations      | 1.22       | 8  | 818.88   | 822.07   | 825.19   | —        | —        | 812.92   | 826.35   | 3.90x                         | 3.90x    |
| tsv-wasm+locations | 1.18       | 8  | 843.24   | 847.66   | 849.59   | —        | —        | 836.41   | 849.77   | 3.79x                         | 3.79x    |
| tsv-internal       | 12.35      | 62 | 80.90    | 81.18    | 81.39    | 81.63    | 81.89    | 80.51    | 81.93    | 39.5x                         | 39.5x    |
| tsv-wasm-internal  | 8.04       | 41 | 124.27   | 124.58   | 124.88   | 125.06   | 125.12   | 123.85   | 125.14   | 25.7x                         | 25.7x    |
| oxc-parser         | 1.13       | 8  | 887.16   | 890.23   | 895.83   | —        | —        | 881.34   | 897.87   | 3.60x                         | 3.60x    |
| oxc-parser-wasm    | 0.87       | 8  | 1144.23  | 1159.47  | 1170.50  | —        | —        | 1107.44  | 1178.41  | 2.80x                         | 2.79x    |
| yuku-parser        | 3.04       | 15 | 324.45   | 343.12   | 344.31   | 350.08   | 363.41   | 317.57   | 366.75   | 9.73x                         | 9.85x    |
| yuku-parser-wasm   | 4.08       | 19 | 244.72   | 257.55   | 262.22   | 287.31   | 289.16   | 237.23   | 289.62   | 13.0x                         | 13.1x    |
| swc                | 0.74       | 8  | 1345.63  | 1349.77  | 1352.55  | —        | —        | 1328.50  | 1355.89  | 2.38x                         | 2.38x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 5.8 MB/s, tsv 30.1 MB/s, tsv-wasm 29.1 MB/s, tsv+locations 22.5 MB/s, tsv-wasm+locations 21.8 MB/s, tsv-internal 227.5 MB/s, tsv-wasm-internal 148.2 MB/s, oxc-parser 20.8 MB/s, oxc-parser-wasm 16.1 MB/s, yuku-parser 56.1 MB/s, yuku-parser-wasm 75.1 MB/s, swc 13.7 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.5x tsv-internal, tsv-wasm 5.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 15.18   | 15.25   | 15.32   | 15.36   | 15.37   | 14.88   | 15.37   | baseline              | baseline |
| tsv         | 2.57       | 13 | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 39.1x                 | 39.1x    |
| tsv-wasm    | 1.82       | 10 | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 27.6x                 | 27.6x    |
| oxfmt       | 1.19       | 7  | 0.84    | 0.85    | 0.86    | —       | —       | 0.83    | 0.88    | 18.0x                 | 18.0x    |
| biome-wasm  | 0.23       | 7  | 4.29    | 4.31    | 4.34    | —       | —       | 4.25    | 4.40    | 3.54x                 | 3.54x    |
| dprint-wasm | 0.31       | 6  | 3.23    | 3.23    | 3.23    | —       | —       | 3.23    | 3.23    | 4.70x                 | 4.70x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.2 MB/s, tsv 47.4 MB/s, tsv-wasm 33.5 MB/s, oxfmt 21.9 MB/s, biome-wasm 4.3 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 49.29      | 228  | 20.29    | 20.42    | 20.57    | 20.98    | 22.67    | 19.77    | 23.05    | baseline                     | baseline |
| tsv                | 70.44      | 288  | 14.15    | 14.54    | 15.08    | 15.65    | 16.19    | 13.92    | 18.47    | 1.43x                        | 1.43x    |
| tsv-wasm           | 77.73      | 339  | 12.86    | 12.94    | 13.23    | 13.63    | 13.94    | 12.70    | 17.12    | 1.58x                        | 1.58x    |
| tsv+locations      | 54.11      | 244  | 18.43    | 18.82    | 19.58    | 19.86    | 20.74    | 18.00    | 22.99    | 1.10x                        | 1.10x    |
| tsv-wasm+locations | 58.78      | 266  | 17.02    | 17.07    | 17.17    | 17.63    | 18.58    | 16.80    | 21.22    | 1.19x                        | 1.19x    |
| tsv-internal       | 428.82     | 1970 | 2.33     | 2.34     | 2.35     | 2.36     | 2.40     | 2.32     | 2.51     | 8.70x                        | 8.70x    |
| tsv-wasm-internal  | 298.59     | 1379 | 3.35     | 3.35     | 3.36     | 3.39     | 3.45     | 3.33     | 3.54     | 6.06x                        | 6.06x    |
| postcss            | 61.07      | 258  | 16.38    | 16.55    | 17.34    | 17.42    | 20.54    | 15.97    | 21.62    | 1.24x                        | 1.24x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.4 MB/s, tsv 27.7 MB/s, tsv-wasm 30.5 MB/s, tsv+locations 21.3 MB/s, tsv-wasm+locations 23.1 MB/s, tsv-internal 168.5 MB/s, tsv-wasm-internal 117.3 MB/s, postcss 24.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.1x tsv-internal, tsv-wasm 3.8x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.04       | 16  | 488.67   | 492.89   | 498.34   | 501.68   | 502.48   | 482.44   | 502.68   | baseline              | baseline |
| tsv        | 205.31     | 947 | 4.87     | 4.89     | 4.94     | 4.97     | 5.19     | 4.82     | 8.87     | 100.5x                | 100.4x   |
| tsv-wasm   | 153.43     | 651 | 6.52     | 6.53     | 6.61     | 6.65     | 6.80     | 6.48     | 7.04     | 75.1x                 | 75.0x    |
| oxfmt      | 55.07      | 268 | 18.13    | 18.48    | 18.88    | 19.22    | 21.71    | 16.92    | 22.28    | 27.0x                 | 26.9x    |
| biome-wasm | 12.50      | 50  | 79.69    | 81.12    | 82.17    | 82.28    | 82.65    | 78.65    | 82.77    | 6.12x                 | 6.13x    |
| malva-wasm | 17.75      | 88  | 56.35    | 56.52    | 56.81    | 57.02    | 57.36    | 55.90    | 57.67    | 8.69x                 | 8.67x    |

**Files (intersection):** 54

**Throughput:** prettier 0.7 MB/s, tsv 72.7 MB/s, tsv-wasm 54.3 MB/s, oxfmt 19.5 MB/s, biome-wasm 4.4 MB/s, malva-wasm 6.3 MB/s

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
| format svelte (923f) | **62.2x** prettier, **65.8x** oxfmt |
| format typescript (2593f) | **39.1x** prettier, **2.17x** oxfmt |
| format css (54f) | **100.5x** prettier, **3.73x** oxfmt |
| parse svelte (929f) | **4.66x** svelte/compiler, **4.72x** rsvelte-parse |
| parse typescript (2593f) | **5.23x** acorn-typescript, **1.45x** oxc-parser, **0.54x** yuku-parser, **2.20x** swc |
| parse css (55f) | **1.43x** svelte/compiler, **1.15x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **44.1x** prettier, **11.1x** biome-wasm |
| format typescript (2593f) | **27.6x** prettier, **7.80x** biome-wasm, **5.87x** dprint-wasm |
| format css (54f) | **75.1x** prettier, **12.3x** biome-wasm, **8.65x** malva-wasm |
| parse svelte (929f) | **4.28x** svelte/compiler |
| parse typescript (2593f) | **5.05x** acorn-typescript, **1.81x** oxc-parser-wasm, **0.39x** yuku-parser-wasm |
| parse css (55f) | **1.58x** svelte/compiler, **1.27x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.41x, wasm 1.41x | typescript: native 1.34x, wasm 1.33x | css: native 1.30x, wasm 1.32x

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
