# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-02T17:23:19.689Z — tsv 0.5.0 (120567d4)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@446268fab](https://github.com/fuzdev/corpora/tree/446268faba0afa32eeed2d35ae53f6b8d3ae4701) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.26       | 16  | 789.87   | 815.28   | 826.17   | 832.23   | 837.47   | 765.83   | 838.78   | baseline                     | baseline |
| tsv-json                    | 6.49       | 33  | 153.97   | 154.46   | 156.41   | 156.70   | 156.99   | 151.72   | 157.12   | 5.17x                        | 5.13x    |
| tsv-wasm-json               | 6.45       | 30  | 155.07   | 155.81   | 157.41   | 159.04   | 159.99   | 153.03   | 160.03   | 5.13x                        | 5.09x    |
| tsv-json-no-locations       | 9.46       | 46  | 105.68   | 106.44   | 107.25   | 107.60   | 109.35   | 104.09   | 109.67   | 7.53x                        | 7.47x    |
| tsv-wasm-json-no-locations  | 8.75       | 42  | 114.33   | 114.88   | 115.30   | 116.52   | 117.97   | 112.66   | 118.47   | 6.96x                        | 6.91x    |
| tsv-internal                | 68.09      | 300 | 14.69    | 14.72    | 14.83    | 14.93    | 15.30    | 14.58    | 15.43    | 54.2x                        | 53.8x    |
| tsv-wasm-internal           | 42.71      | 195 | 23.41    | 23.47    | 23.60    | 23.89    | 24.25    | 23.28    | 24.31    | 34.0x                        | 33.7x    |
| rsvelte-parse               | 2.05       | 11  | 485.90   | 490.16   | 494.23   | 497.33   | 499.82   | 481.04   | 500.44   | 1.63x                        | 1.63x    |
| rsvelte-parse-skip-expr-loc | 3.06       | 14  | 327.21   | 329.34   | 337.66   | 340.54   | 341.52   | 321.72   | 341.76   | 2.43x                        | 2.41x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.0 MB/s, tsv-json 15.7 MB/s, tsv-wasm-json 15.6 MB/s, tsv-json-no-locations 22.8 MB/s, tsv-wasm-json-no-locations 21.1 MB/s, tsv-internal 164.2 MB/s, tsv-wasm-internal 103.0 MB/s, rsvelte-parse 4.9 MB/s, rsvelte-parse-skip-expr-loc 7.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.5x tsv-internal, tsv-wasm-json 6.6x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.24       | 13 | 4.13    | 4.16    | 4.35    | 4.46    | 4.54    | 4.09    | 4.56    | baseline              | baseline |
| tsv        | 14.58      | 68 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 60.1x                 | 60.2x    |
| tsv-wasm   | 10.35      | 49 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 42.6x                 | 42.7x    |
| oxfmt      | 0.22       | 6  | 4.48    | 4.53    | 4.65    | —       | —       | 4.45    | 4.74    | 0.92x                 | 0.92x    |
| biome-wasm | 0.93       | 8  | 1.08    | 1.09    | 1.10    | —       | —       | 1.04    | 1.11    | 3.84x                 | 3.82x    |

**Files (intersection):** 945

**Throughput:** prettier 0.6 MB/s, tsv 34.0 MB/s, tsv-wasm 24.1 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript           | 0.18       | 16 | 5396.37  | 5439.39  | 5469.27  | 5480.30  | 5498.88  | 5338.62  | 5503.52  | baseline                      | baseline |
| tsv-json                   | 0.92       | 8  | 1087.18  | 1091.10  | 1092.64  | —        | —        | 1074.11  | 1092.65  | 4.98x                         | 4.96x    |
| tsv-wasm-json              | 0.95       | 8  | 1048.40  | 1053.64  | 1054.04  | —        | —        | 1045.90  | 1054.80  | 5.15x                         | 5.15x    |
| tsv-json-no-locations      | 1.63       | 9  | 613.06   | 615.35   | 618.18   | —        | —        | 607.08   | 620.40   | 8.81x                         | 8.80x    |
| tsv-wasm-json-no-locations | 1.58       | 8  | 633.98   | 636.71   | 640.18   | —        | —        | 629.95   | 641.33   | 8.52x                         | 8.51x    |
| tsv-internal               | 12.16      | 58 | 82.14    | 82.53    | 82.87    | 83.19    | 83.23    | 81.47    | 83.24    | 65.7x                         | 65.7x    |
| tsv-wasm-internal          | 8.14       | 41 | 122.90   | 123.05   | 123.51   | 123.57   | 123.75   | 122.46   | 123.84   | 44.0x                         | 43.9x    |
| oxc-parser                 | 1.14       | 8  | 877.42   | 878.63   | 878.98   | —        | —        | 872.42   | 879.31   | 6.17x                         | 6.15x    |
| oxc-parser-wasm            | 0.86       | 8  | 1162.81  | 1169.01  | 1176.94  | —        | —        | 1117.44  | 1180.90  | 4.67x                         | 4.64x    |
| yuku-parser                | 2.88       | 13 | 347.66   | 357.18   | 385.91   | 397.95   | 398.62   | 337.98   | 398.78   | 15.6x                         | 15.5x    |
| yuku-parser-wasm           | 3.48       | 16 | 287.07   | 294.76   | 311.53   | 335.73   | 363.64   | 277.89   | 370.62   | 18.8x                         | 18.8x    |
| swc                        | 0.74       | 8  | 1349.88  | 1352.72  | 1354.08  | —        | —        | 1340.63  | 1356.22  | 4.01x                         | 4.00x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.4 MB/s, tsv-json 17.0 MB/s, tsv-wasm-json 17.6 MB/s, tsv-json-no-locations 30.0 MB/s, tsv-wasm-json-no-locations 29.0 MB/s, tsv-internal 224.2 MB/s, tsv-wasm-internal 150.0 MB/s, oxc-parser 21.0 MB/s, oxc-parser-wasm 15.9 MB/s, yuku-parser 53.1 MB/s, yuku-parser-wasm 64.2 MB/s, swc 13.7 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.2x tsv-internal, tsv-wasm-json 8.5x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 13.80   | 13.88   | 13.96   | 13.99   | 14.01   | 13.44   | 14.01   | baseline              | baseline |
| tsv         | 2.56       | 13 | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 35.2x                 | 35.3x    |
| tsv-wasm    | 1.82       | 10 | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 25.1x                 | 25.2x    |
| oxfmt       | 1.10       | 6  | 0.91    | 0.91    | 0.92    | —       | —       | 0.90    | 0.94    | 15.2x                 | 15.2x    |
| biome-wasm  | 0.23       | 8  | 4.38    | 4.39    | 4.42    | —       | —       | 4.33    | 4.47    | 3.14x                 | 3.15x    |
| dprint-wasm | 0.31       | 8  | 3.23    | 3.23    | 3.24    | —       | —       | 3.22    | 3.24    | 4.27x                 | 4.27x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.3 MB/s, tsv 47.2 MB/s, tsv-wasm 33.6 MB/s, oxfmt 20.4 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 49.45      | 226  | 20.18    | 20.52    | 20.92    | 21.08    | 23.03    | 19.66    | 23.49    | baseline                     | baseline |
| tsv-json          | 70.37      | 309  | 14.13    | 14.59    | 15.10    | 15.57    | 16.99    | 13.79    | 18.61    | 1.42x                        | 1.43x    |
| tsv-wasm-json     | 77.89      | 319  | 12.83    | 13.05    | 13.58    | 13.77    | 14.35    | 12.63    | 17.56    | 1.58x                        | 1.57x    |
| tsv-internal      | 438.92     | 2081 | 2.28     | 2.29     | 2.29     | 2.31     | 2.36     | 2.26     | 6.31     | 8.88x                        | 8.86x    |
| tsv-wasm-internal | 299.57     | 1277 | 3.34     | 3.35     | 3.37     | 3.39     | 3.45     | 3.32     | 3.55     | 6.06x                        | 6.05x    |
| postcss           | 71.72      | 288  | 13.92    | 14.32    | 14.89    | 15.09    | 18.00    | 13.55    | 18.26    | 1.45x                        | 1.45x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.4 MB/s, tsv-json 27.7 MB/s, tsv-wasm-json 30.6 MB/s, tsv-internal 172.5 MB/s, tsv-wasm-internal 117.7 MB/s, postcss 28.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 6.2x tsv-internal, tsv-wasm-json 3.8x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.35       | 16  | 425.68   | 429.07   | 433.71   | 435.30   | 435.94   | 412.46   | 436.11   | baseline              | baseline |
| tsv        | 210.80     | 941 | 4.74     | 4.76     | 4.83     | 4.88     | 5.10     | 4.69     | 6.11     | 89.7x                 | 89.8x    |
| tsv-wasm   | 153.99     | 662 | 6.49     | 6.51     | 6.59     | 6.62     | 6.82     | 6.46     | 10.76    | 65.5x                 | 65.5x    |
| oxfmt      | 54.11      | 263 | 18.45    | 18.92    | 19.43    | 19.98    | 21.63    | 16.81    | 23.23    | 23.0x                 | 23.1x    |
| biome-wasm | 12.19      | 48  | 81.80    | 82.53    | 84.03    | 84.59    | 98.37    | 81.06    | 111.52   | 5.18x                 | 5.20x    |
| malva-wasm | 17.92      | 79  | 55.79    | 55.92    | 56.26    | 56.36    | 56.56    | 55.62    | 56.62    | 7.62x                 | 7.63x    |

**Files (intersection):** 54

**Throughput:** prettier 0.8 MB/s, tsv 74.6 MB/s, tsv-wasm 54.5 MB/s, oxfmt 19.2 MB/s, biome-wasm 4.3 MB/s, malva-wasm 6.3 MB/s

**Coverage:** prettier 55/55 (100%), tsv 55/55 (100%), tsv-wasm 55/55 (100%), oxfmt 55/55 (100%), biome-wasm 54/55 (98%), malva-wasm 55/55 (100%)

**Omitted from every row's timed set:** 1 of 55 files, 9.9% of the group's bytes (1.8% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.4 MB | 937.0 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.0 MB | 398.9 KB | 0.4x | 0.4x |
| tsv-wasm | 2.7 MB | 1.0 MB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 16.5x | 10.9x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.6 MB | 1.7 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 702.2 KB | 0.4x | 0.4x |
| tsv (napi) | 4.0 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.8x | 2.5x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.5x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.3x | 2.0x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.3x | 1.9x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.5x | 4.0x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.3x | 6.7x |
| svelte + acorn-typescript parsers (js bundle) | 497.2 KB | 124.0 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 566.1 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 566.3 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **60.1x** prettier, **65.2x** oxfmt |
| format typescript (2642f) | **35.2x** prettier, **2.31x** oxfmt |
| format css (54f) | **89.7x** prettier, **3.90x** oxfmt |
| parse svelte (951f) | **5.17x** svelte/compiler, **3.16x** rsvelte-parse |
| parse typescript (2642f) | **4.98x** acorn-typescript, **0.81x** oxc-parser, **0.32x** yuku-parser, **1.24x** swc |
| parse css (55f) | **1.42x** svelte/compiler, **0.98x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **42.6x** prettier, **11.1x** biome-wasm |
| format typescript (2642f) | **25.1x** prettier, **7.99x** biome-wasm, **5.89x** dprint-wasm |
| format css (54f) | **65.5x** prettier, **12.6x** biome-wasm, **8.59x** malva-wasm |
| parse svelte (951f) | **5.13x** svelte/compiler |
| parse typescript (2642f) | **5.15x** acorn-typescript, **1.10x** oxc-parser-wasm, **0.27x** yuku-parser-wasm |
| parse css (55f) | **1.58x** svelte/compiler, **1.09x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are mechanism-matched; the payload is not: oxc’s default AST is span-only (`start`/`end`, no per-node `loc`, and no option to add one) where `tsv-json` carries the loc-bearing drop-in AST, so the payload-matched read is the `no-locations` line under each parse group. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and its payload-matched read is the same `no-locations` line), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

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
