# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.7

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-10T16:54:30.127Z — tsv 0.6.0 (c93c1649)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.5, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 1.9% at the 95th percentile and 6.0% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 3.04       | 46  | 329.15   | 330.95   | 335.03   | 338.33   | 339.81   | 322.11   | 340.26   | baseline                     | baseline |
| tsv                         | 7.65       | 110 | 130.73   | 131.30   | 132.08   | 132.83   | 141.64   | 129.22   | 147.21   | 2.52x                        | 2.52x    |
| tsv-wasm                    | 6.22       | 89  | 160.85   | 161.34   | 162.30   | 166.08   | 172.94   | 159.47   | 173.91   | 2.05x                        | 2.05x    |
| tsv+locations               | 5.42       | 81  | 184.76   | 186.54   | 187.32   | 188.00   | 188.41   | 181.31   | 188.51   | 1.78x                        | 1.78x    |
| tsv-wasm+locations          | 4.66       | 72  | 214.53   | 215.60   | 216.03   | 216.44   | 217.81   | 212.85   | 217.98   | 1.53x                        | 1.53x    |
| tsv-internal                | 62.23      | 808 | 16.08    | 16.16    | 16.33    | 16.46    | 16.72    | 15.88    | 17.24    | 20.5x                        | 20.5x    |
| tsv-wasm-internal           | 36.75      | 502 | 27.26    | 27.33    | 27.47    | 27.65    | 28.13    | 26.90    | 28.28    | 12.1x                        | 12.1x    |
| rsvelte-parse               | 1.79       | 27  | 558.81   | 559.96   | 560.56   | 560.90   | 561.88   | 556.13   | 562.19   | 0.59x                        | 0.59x    |
| rsvelte-parse-skip-expr-loc | 2.75       | 42  | 363.89   | 365.12   | 366.23   | 366.37   | 367.64   | 360.21   | 368.39   | 0.90x                        | 0.90x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 7.3 MB/s, tsv 18.5 MB/s, tsv-wasm 15.0 MB/s, tsv+locations 13.1 MB/s, tsv-wasm+locations 11.2 MB/s, tsv-internal 150.1 MB/s, tsv-wasm-internal 88.6 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.1x tsv-internal, tsv-wasm 5.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.20       | 24  | 5056.45  | 5083.04  | 5105.22  | 5114.95  | 5121.34  | 4936.27  | 5122.87  | baseline              | baseline |
| tsv        | 14.94      | 212 | 66.85    | 67.23    | 67.76    | 68.38    | 69.10    | 66.36    | 71.54    | 75.4x                 | 75.6x    |
| tsv-wasm   | 9.23       | 126 | 108.17   | 109.11   | 109.58   | 110.02   | 110.51   | 106.56   | 110.73   | 46.5x                 | 46.7x    |
| oxfmt      | 0.20       | 23  | 5092.45  | 5113.02  | 5139.73  | 5144.59  | 5145.48  | 5054.66  | 5145.55  | 0.99x                 | 0.99x    |
| biome-wasm | 1.08       | 24  | 929.32   | 932.29   | 936.56   | 936.98   | 938.17   | 916.57   | 938.52   | 5.43x                 | 5.44x    |

**Files (intersection):** 923

**Throughput:** prettier 0.5 MB/s, tsv 34.8 MB/s, tsv-wasm 21.5 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 868/929 (93%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.97       | 24  | 1.04    | 1.04    | 1.04    | 1.04    | 1.05    | 1.03    | 1.05    | baseline                      | baseline |
| tsv                | 1.27       | 21  | 0.79    | 0.79    | 0.79    | 0.79    | 0.79    | 0.79    | 0.79    | 1.31x                         | 1.31x    |
| tsv-wasm           | 1.05       | 20  | 0.95    | 0.96    | 0.96    | 0.96    | 0.96    | 0.95    | 0.96    | 1.09x                         | 1.09x    |
| tsv+locations      | 1.00       | 23  | 1.00    | 1.01    | 1.01    | 1.01    | 1.02    | 0.99    | 1.02    | 1.03x                         | 1.03x    |
| tsv-wasm+locations | 0.86       | 23  | 1.16    | 1.17    | 1.17    | 1.18    | 1.18    | 1.15    | 1.18    | 0.89x                         | 0.89x    |
| tsv-internal       | 10.95      | 158 | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 11.3x                         | 11.3x    |
| tsv-wasm-internal  | 6.42       | 98  | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.15    | 0.16    | 6.65x                         | 6.64x    |
| oxc-parser         | 0.82       | 24  | 1.21    | 1.22    | 1.23    | 1.23    | 1.23    | 1.20    | 1.23    | 0.85x                         | 0.85x    |
| oxc-parser-wasm    | 0.74       | 24  | 1.35    | 1.35    | 1.36    | 1.36    | 1.36    | 1.34    | 1.36    | 0.77x                         | 0.77x    |
| yuku-parser        | 2.28       | 28  | 0.44    | 0.46    | 0.46    | 0.46    | 0.48    | 0.43    | 0.48    | 2.36x                         | 2.36x    |
| yuku-parser-wasm   | 2.65       | 33  | 0.38    | 0.38    | 0.40    | 0.40    | 0.40    | 0.37    | 0.40    | 2.74x                         | 2.73x    |
| swc                | 0.62       | 21  | 1.60    | 1.61    | 1.62    | 1.62    | 1.62    | 1.60    | 1.62    | 0.65x                         | 0.65x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 17.8 MB/s, tsv 23.3 MB/s, tsv-wasm 19.3 MB/s, tsv+locations 18.4 MB/s, tsv-wasm+locations 15.8 MB/s, tsv-internal 201.8 MB/s, tsv-wasm-internal 118.4 MB/s, oxc-parser 15.2 MB/s, oxc-parser-wasm 13.7 MB/s, yuku-parser 41.9 MB/s, yuku-parser-wasm 48.7 MB/s, swc 11.5 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.6x tsv-internal, tsv-wasm 6.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 23 | 13.15   | 13.78   | 13.86   | 13.90   | 13.93   | 12.99   | 13.93   | baseline              | baseline |
| tsv         | 2.63       | 41 | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 0.38    | 35.2x                 | 34.6x    |
| tsv-wasm    | 1.59       | 19 | 0.63    | 0.63    | 0.63    | 0.63    | 0.63    | 0.63    | 0.63    | 21.3x                 | 20.9x    |
| oxfmt       | 1.18       | 22 | 0.84    | 0.85    | 0.86    | 0.86    | 0.86    | 0.83    | 0.86    | 15.8x                 | 15.6x    |
| biome-wasm  | 0.23       | 24 | 4.38    | 4.40    | 4.41    | 4.42    | 4.43    | 4.35    | 4.43    | 3.04x                 | 3.00x    |
| dprint-wasm | 0.27       | 24 | 3.73    | 3.73    | 3.75    | 3.75    | 3.75    | 3.71    | 3.75    | 3.58x                 | 3.53x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.4 MB/s, tsv 48.5 MB/s, tsv-wasm 29.3 MB/s, oxfmt 21.8 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 4.9 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 98.32      | 1460 | 10.14    | 10.30    | 10.63    | 10.84    | 11.13    | 9.72     | 13.69    | baseline                     | baseline |
| tsv                | 60.80      | 874  | 16.42    | 16.56    | 16.74    | 16.90    | 17.27    | 16.11    | 19.73    | 0.62x                        | 0.62x    |
| tsv-wasm           | 49.40      | 721  | 20.23    | 20.38    | 20.48    | 20.62    | 21.01    | 19.87    | 24.58    | 0.50x                        | 0.50x    |
| tsv+locations      | 47.88      | 577  | 20.91    | 21.38    | 22.71    | 22.87    | 23.38    | 20.33    | 30.94    | 0.49x                        | 0.48x    |
| tsv-wasm+locations | 40.73      | 482  | 24.58    | 25.15    | 26.45    | 26.60    | 27.05    | 23.99    | 34.20    | 0.41x                        | 0.41x    |
| tsv-internal       | 396.30     | 5595 | 2.52     | 2.54     | 2.56     | 2.58     | 2.67     | 2.48     | 4.65     | 4.03x                        | 4.02x    |
| tsv-wasm-internal  | 220.47     | 3124 | 4.53     | 4.58     | 4.61     | 4.64     | 4.72     | 4.46     | 6.02     | 2.24x                        | 2.24x    |
| postcss            | 82.52      | 1222 | 12.06    | 12.34    | 12.64    | 12.79    | 13.41    | 11.59    | 15.97    | 0.84x                        | 0.84x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 38.6 MB/s, tsv 23.9 MB/s, tsv-wasm 19.4 MB/s, tsv+locations 18.8 MB/s, tsv-wasm+locations 16.0 MB/s, tsv-internal 155.7 MB/s, tsv-wasm-internal 86.6 MB/s, postcss 32.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.5x tsv-internal, tsv-wasm 4.5x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.82       | 29   | 550.08   | 557.06   | 563.65   | 566.91   | 567.82   | 529.04   | 568.13   | baseline              | baseline |
| tsv        | 218.89     | 2866 | 4.57     | 4.59     | 4.65     | 4.71     | 4.85     | 4.52     | 8.95     | 120.3x                | 120.4x   |
| tsv-wasm   | 120.29     | 1595 | 8.31     | 8.36     | 8.43     | 8.48     | 8.65     | 8.24     | 12.64    | 66.1x                 | 66.2x    |
| oxfmt      | 55.27      | 815  | 18.08    | 18.36    | 18.64    | 18.85    | 19.66    | 16.90    | 20.51    | 30.4x                 | 30.4x    |
| biome-wasm | 10.57      | 133  | 94.45    | 95.78    | 96.58    | 100.52   | 102.93   | 93.64    | 103.49   | 5.81x                 | 5.82x    |
| malva-wasm | 19.44      | 256  | 51.43    | 51.62    | 51.99    | 52.20    | 52.64    | 51.05    | 55.47    | 10.7x                 | 10.7x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 77.5 MB/s, tsv-wasm 42.6 MB/s, oxfmt 19.6 MB/s, biome-wasm 3.7 MB/s, malva-wasm 6.9 MB/s

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
| format svelte (923f) | **75.4x** prettier, **76.2x** oxfmt |
| format typescript (2593f) | **35.2x** prettier, **2.23x** oxfmt |
| format css (54f) | **120.3x** prettier, **3.96x** oxfmt |
| parse svelte (929f) | **2.52x** svelte/compiler, **4.28x** rsvelte-parse |
| parse typescript (2593f) | **1.31x** acorn-typescript, **1.54x** oxc-parser, **0.56x** yuku-parser, **2.03x** swc |
| parse css (55f) | **0.62x** svelte/compiler, **0.74x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **46.5x** prettier, **8.57x** biome-wasm |
| format typescript (2593f) | **21.3x** prettier, **6.98x** biome-wasm, **5.93x** dprint-wasm |
| format css (54f) | **66.1x** prettier, **11.4x** biome-wasm, **6.19x** malva-wasm |
| parse svelte (929f) | **2.05x** svelte/compiler |
| parse typescript (2593f) | **1.09x** acorn-typescript, **1.41x** oxc-parser-wasm, **0.40x** yuku-parser-wasm |
| parse css (55f) | **0.50x** svelte/compiler, **0.60x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.41x, wasm 1.34x | typescript: native 1.27x, wasm 1.22x | css: native 1.27x, wasm 1.21x

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| format/typescript/prettier | 2.7% | 2.7% | -0.2% | 6.0% | 23/24 |

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
