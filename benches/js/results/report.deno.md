# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-02T17:53:31.615Z — tsv 0.5.0 (120567d4)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@446268fab](https://github.com/fuzdev/corpora/tree/446268faba0afa32eeed2d35ae53f6b8d3ae4701) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.63       | 16  | 612.45   | 617.62   | 623.17   | 629.63   | 641.55   | 597.99   | 644.53   | baseline                     | baseline |
| tsv-json                    | 4.34       | 20  | 230.14   | 230.77   | 231.93   | 233.96   | 236.83   | 229.50   | 237.57   | 2.66x                        | 2.66x    |
| tsv-wasm-json               | 3.73       | 17  | 267.94   | 268.29   | 269.50   | 272.02   | 272.60   | 267.22   | 272.75   | 2.29x                        | 2.29x    |
| tsv-json-no-locations       | 7.14       | 34  | 139.82   | 140.61   | 140.89   | 141.38   | 143.37   | 139.29   | 144.13   | 4.38x                        | 4.38x    |
| tsv-wasm-json-no-locations  | 5.82       | 30  | 171.82   | 172.27   | 172.50   | 172.74   | 173.14   | 171.22   | 173.25   | 3.57x                        | 3.56x    |
| tsv-internal                | 62.19      | 284 | 16.08    | 16.11    | 16.16    | 16.33    | 16.58    | 16.00    | 16.61    | 38.1x                        | 38.1x    |
| tsv-wasm-internal           | 35.73      | 165 | 27.99    | 28.03    | 28.15    | 28.39    | 28.86    | 27.88    | 32.28    | 21.9x                        | 21.9x    |
| rsvelte-parse               | 1.79       | 9   | 559.92   | 560.50   | 561.83   | —        | —        | 556.03   | 562.96   | 1.10x                        | 1.09x    |
| rsvelte-parse-skip-expr-loc | 2.73       | 14  | 365.99   | 366.90   | 368.85   | 369.74   | 370.26   | 363.51   | 370.38   | 1.67x                        | 1.67x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 10.5 MB/s, tsv-wasm-json 9.0 MB/s, tsv-json-no-locations 17.2 MB/s, tsv-wasm-json-no-locations 14.0 MB/s, tsv-internal 150.0 MB/s, tsv-wasm-internal 86.2 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.3x tsv-internal, tsv-wasm-json 9.6x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.19       | 16 | 5203.09  | 5236.67  | 5280.84  | 5325.37  | 5330.06  | 5112.94  | 5331.24  | baseline              | baseline |
| tsv        | 15.00      | 72 | 66.55    | 66.91    | 67.14    | 67.28    | 67.82    | 66.33    | 67.90    | 78.1x                 | 78.2x    |
| tsv-wasm   | 9.25       | 38 | 108.01   | 108.77   | 109.51   | 110.79   | 111.42   | 107.70   | 111.57   | 48.2x                 | 48.2x    |
| oxfmt      | 0.19       | 8  | 5274.87  | 5299.66  | 5323.88  | —        | —        | 5213.00  | 5326.85  | 0.99x                 | 0.99x    |
| biome-wasm | 1.10       | 8  | 906.97   | 911.36   | 916.48   | —        | —        | 898.63   | 928.25   | 5.73x                 | 5.74x    |

**Files (intersection):** 945

**Throughput:** prettier 0.4 MB/s, tsv 34.9 MB/s, tsv-wasm 21.5 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript           | 0.30       | 16 | 3.30    | 3.31    | 3.33    | 3.34    | 3.34    | 3.26    | 3.34    | baseline                      | baseline |
| tsv-json                   | 0.55       | 8  | 1.82    | 1.83    | 1.84    | —       | —       | 1.82    | 1.84    | 1.81x                         | 1.81x    |
| tsv-wasm-json              | 0.49       | 8  | 2.04    | 2.05    | 2.05    | —       | —       | 2.03    | 2.05    | 1.62x                         | 1.62x    |
| tsv-json-no-locations      | 1.16       | 7  | 0.86    | 0.86    | 0.86    | —       | —       | 0.86    | 0.87    | 3.84x                         | 3.84x    |
| tsv-wasm-json-no-locations | 0.98       | 7  | 1.02    | 1.03    | 1.03    | —       | —       | 1.02    | 1.03    | 3.22x                         | 3.22x    |
| tsv-internal               | 11.15      | 51 | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 36.8x                         | 36.8x    |
| tsv-wasm-internal          | 6.41       | 31 | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 21.1x                         | 21.1x    |
| oxc-parser                 | 0.80       | 8  | 1.26    | 1.27    | 1.27    | —       | —       | 1.23    | 1.27    | 2.63x                         | 2.62x    |
| oxc-parser-wasm            | 0.72       | 7  | 1.38    | 1.39    | 1.39    | —       | —       | 1.38    | 1.40    | 2.39x                         | 2.39x    |
| yuku-parser                | 2.11       | 9  | 0.48    | 0.48    | 0.48    | —       | —       | 0.46    | 0.51    | 6.96x                         | 6.90x    |
| yuku-parser-wasm           | 2.31       | 11 | 0.43    | 0.44    | 0.48    | 0.52    | 0.55    | 0.41    | 0.56    | 7.61x                         | 7.58x    |
| swc                        | 0.58       | 8  | 1.71    | 1.72    | 1.72    | —       | —       | 1.71    | 1.72    | 1.92x                         | 1.92x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 10.1 MB/s, tsv-wasm-json 9.0 MB/s, tsv-json-no-locations 21.5 MB/s, tsv-wasm-json-no-locations 18.0 MB/s, tsv-internal 205.7 MB/s, tsv-wasm-internal 118.2 MB/s, oxc-parser 14.7 MB/s, oxc-parser-wasm 13.3 MB/s, yuku-parser 38.9 MB/s, yuku-parser-wasm 42.6 MB/s, swc 10.8 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 20.4x tsv-internal, tsv-wasm-json 13.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.08       | 15 | 13.24   | 13.26   | 13.29   | 13.33   | 13.39   | 13.13   | 13.41   | baseline              | baseline |
| tsv         | 2.68       | 14 | 0.37    | 0.37    | 0.37    | 0.37    | 0.37    | 0.37    | 0.37    | 35.4x                 | 35.5x    |
| tsv-wasm    | 1.61       | 8  | 0.62    | 0.62    | 0.62    | —       | —       | 0.62    | 0.63    | 21.3x                 | 21.4x    |
| oxfmt       | 1.10       | 8  | 0.91    | 0.91    | 0.92    | —       | —       | 0.91    | 0.92    | 14.5x                 | 14.5x    |
| biome-wasm  | 0.22       | 8  | 4.59    | 4.60    | 4.60    | —       | —       | 4.58    | 4.61    | 2.88x                 | 2.88x    |
| dprint-wasm | 0.27       | 7  | 3.71    | 3.72    | 3.72    | —       | —       | 3.71    | 3.73    | 3.56x                 | 3.57x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.4 MB/s, tsv 49.4 MB/s, tsv-wasm 29.8 MB/s, oxfmt 20.2 MB/s, biome-wasm 4.0 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 75.88      | 380  | 13.14    | 13.39    | 13.51    | 13.57    | 13.68    | 12.75    | 14.12    | baseline                     | baseline |
| tsv-json          | 54.89      | 222  | 18.20    | 18.45    | 18.87    | 19.67    | 20.45    | 18.02    | 30.30    | 0.72x                        | 0.72x    |
| tsv-wasm-json     | 46.45      | 206  | 21.53    | 21.72    | 22.44    | 23.38    | 24.68    | 21.13    | 32.40    | 0.61x                        | 0.61x    |
| tsv-internal      | 397.19     | 1862 | 2.52     | 2.52     | 2.54     | 2.55     | 2.60     | 2.50     | 2.97     | 5.23x                        | 5.22x    |
| tsv-wasm-internal | 216.60     | 1044 | 4.61     | 4.63     | 4.66     | 4.68     | 4.75     | 4.58     | 8.64     | 2.85x                        | 2.85x    |
| postcss           | 83.50      | 418  | 11.92    | 12.19    | 12.39    | 12.57    | 12.79    | 11.52    | 13.23    | 1.10x                        | 1.10x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 29.8 MB/s, tsv-json 21.6 MB/s, tsv-wasm-json 18.3 MB/s, tsv-internal 156.1 MB/s, tsv-wasm-internal 85.1 MB/s, postcss 32.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.2x tsv-internal, tsv-wasm-json 4.7x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.80       | 14   | 555.32   | 557.17   | 572.93   | 587.95   | 593.12   | 548.81   | 594.41   | baseline              | baseline |
| tsv        | 218.40     | 1005 | 4.58     | 4.59     | 4.61     | 4.64     | 4.85     | 4.55     | 4.92     | 121.1x                | 121.3x   |
| tsv-wasm   | 120.35     | 551  | 8.30     | 8.34     | 8.42     | 8.48     | 8.65     | 8.23     | 8.91     | 66.7x                 | 66.9x    |
| oxfmt      | 54.36      | 263  | 18.40    | 18.68    | 19.04    | 19.36    | 20.18    | 16.95    | 21.36    | 30.1x                 | 30.2x    |
| biome-wasm | 10.08      | 45   | 98.87    | 100.49   | 101.16   | 102.32   | 110.11   | 98.06    | 116.88   | 5.59x                 | 5.62x    |
| malva-wasm | 19.55      | 87   | 51.12    | 51.29    | 51.66    | 51.85    | 52.35    | 50.92    | 52.99    | 10.8x                 | 10.9x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 77.3 MB/s, tsv-wasm 42.6 MB/s, oxfmt 19.2 MB/s, biome-wasm 3.6 MB/s, malva-wasm 6.9 MB/s

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
| tsv (ffi) | 3.6 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 2.8x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 702.2 KB | 0.4x | 0.4x |
| tsv (napi) | 4.0 MB | 1.8 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.6x | 2.2x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.5x | 2.1x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 5.0x | 4.4x |
| swc (napi) | 32.7 MB | 12.2 MB | 9.2x | 7.4x |
| svelte + acorn-typescript parsers (js bundle) | 497.2 KB | 124.0 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 566.1 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 566.3 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **78.1x** prettier, **79.1x** oxfmt |
| format typescript (2642f) | **35.4x** prettier, **2.44x** oxfmt |
| format css (54f) | **121.1x** prettier, **4.02x** oxfmt |
| parse svelte (951f) | **2.66x** svelte/compiler, **2.43x** rsvelte-parse |
| parse typescript (2642f) | **1.81x** acorn-typescript, **0.69x** oxc-parser, **0.26x** yuku-parser, **0.94x** swc |
| parse css (55f) | **0.72x** svelte/compiler, **0.66x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **48.2x** prettier, **8.40x** biome-wasm |
| format typescript (2642f) | **21.3x** prettier, **7.41x** biome-wasm, **5.99x** dprint-wasm |
| format css (54f) | **66.7x** prettier, **11.9x** biome-wasm, **6.16x** malva-wasm |
| parse svelte (951f) | **2.29x** svelte/compiler |
| parse typescript (2642f) | **1.62x** acorn-typescript, **0.68x** oxc-parser-wasm, **0.21x** yuku-parser-wasm |
| parse css (55f) | **0.61x** svelte/compiler, **0.56x** postcss |

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
