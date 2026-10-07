# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-07T02:10:58.325Z — tsv 0.6.0 (3a8e53d7)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.11       | 16  | 472.33   | 476.32   | 484.24   | 487.31   | 494.05   | 465.13   | 495.73   | baseline                     | baseline |
| tsv                         | 6.49       | 25  | 154.09   | 154.46   | 154.81   | 155.07   | 155.41   | 153.87   | 155.47   | 3.07x                        | 3.07x    |
| tsv-wasm                    | 5.93       | 29  | 168.63   | 169.03   | 169.67   | 170.37   | 170.60   | 167.01   | 170.69   | 2.81x                        | 2.80x    |
| tsv+locations               | 4.91       | 24  | 203.73   | 204.18   | 204.85   | 204.93   | 205.89   | 202.71   | 206.18   | 2.32x                        | 2.32x    |
| tsv-wasm+locations          | 4.58       | 23  | 218.26   | 218.89   | 219.32   | 219.40   | 219.71   | 217.06   | 219.80   | 2.17x                        | 2.16x    |
| tsv-internal                | 60.40      | 276 | 16.56    | 16.58    | 16.64    | 16.72    | 17.06    | 16.45    | 17.27    | 28.6x                        | 28.5x    |
| tsv-wasm-internal           | 40.97      | 181 | 24.40    | 24.45    | 24.61    | 24.75    | 24.95    | 24.29    | 28.70    | 19.4x                        | 19.4x    |
| rsvelte-parse               | 1.66       | 9   | 600.10   | 601.59   | 603.01   | —        | —        | 598.31   | 604.60   | 0.79x                        | 0.79x    |
| rsvelte-parse-skip-expr-loc | 2.56       | 13  | 389.85   | 391.33   | 392.91   | 393.30   | 393.38   | 388.07   | 393.40   | 1.21x                        | 1.21x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 5.1 MB/s, tsv 15.6 MB/s, tsv-wasm 14.3 MB/s, tsv+locations 11.8 MB/s, tsv-wasm+locations 11.0 MB/s, tsv-internal 145.7 MB/s, tsv-wasm-internal 98.8 MB/s, rsvelte-parse 4.0 MB/s, rsvelte-parse-skip-expr-loc 6.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.3x tsv-internal, tsv-wasm 6.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.19       | 16 | 5.35    | 5.40    | 5.43    | 5.44    | 5.47    | 5.29    | 5.48    | baseline              | baseline |
| tsv        | 14.68      | 55 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 78.8x                 | 78.6x    |
| tsv-wasm   | 10.31      | 40 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 55.3x                 | 55.2x    |
| oxfmt      | 0.19       | 8  | 5.38    | 5.39    | 5.40    | —       | —       | 5.35    | 5.41    | 1.00x                 | 0.99x    |
| biome-wasm | 0.84       | 8  | 1.19    | 1.20    | 1.21    | —       | —       | 1.19    | 1.21    | 4.48x                 | 4.48x    |

**Files (intersection):** 923

**Throughput:** prettier 0.4 MB/s, tsv 34.2 MB/s, tsv-wasm 24.0 MB/s, oxfmt 0.4 MB/s, biome-wasm 1.9 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.34       | 16 | 2.96    | 2.98    | 2.98    | 2.99    | 2.99    | 2.94    | 2.99    | baseline                      | baseline |
| tsv                | 1.01       | 8  | 0.99    | 0.99    | 0.99    | —       | —       | 0.98    | 0.99    | 3.01x                         | 3.01x    |
| tsv-wasm           | 0.96       | 8  | 1.04    | 1.04    | 1.04    | —       | —       | 1.04    | 1.04    | 2.85x                         | 2.85x    |
| tsv+locations      | 0.82       | 8  | 1.22    | 1.22    | 1.22    | —       | —       | 1.22    | 1.22    | 2.43x                         | 2.43x    |
| tsv-wasm+locations | 0.78       | 8  | 1.28    | 1.28    | 1.28    | —       | —       | 1.28    | 1.28    | 2.32x                         | 2.32x    |
| tsv-internal       | 9.80       | 48 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 29.1x                         | 29.1x    |
| tsv-wasm-internal  | 7.37       | 33 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 21.9x                         | 21.9x    |
| oxc-parser         | 0.72       | 8  | 1.39    | 1.40    | 1.41    | —       | —       | 1.38    | 1.41    | 2.13x                         | 2.13x    |
| oxc-parser-wasm    | 0.70       | 7  | 1.43    | 1.43    | 1.44    | —       | —       | 1.43    | 1.45    | 2.07x                         | 2.07x    |
| yuku-parser        | 2.50       | 9  | 0.40    | 0.42    | 0.42    | —       | —       | 0.40    | 0.42    | 7.41x                         | 7.39x    |
| yuku-parser-wasm   | 3.08       | 12 | 0.33    | 0.33    | 0.35    | 0.35    | 0.35    | 0.32    | 0.35    | 9.14x                         | 9.12x    |
| swc                | 0.55       | 8  | 1.83    | 1.83    | 1.83    | —       | —       | 1.83    | 1.84    | 1.62x                         | 1.62x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 6.2 MB/s, tsv 18.7 MB/s, tsv-wasm 17.7 MB/s, tsv+locations 15.1 MB/s, tsv-wasm+locations 14.4 MB/s, tsv-internal 180.6 MB/s, tsv-wasm-internal 135.8 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.9 MB/s, yuku-parser 46.1 MB/s, yuku-parser-wasm 56.8 MB/s, swc 10.1 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.7x tsv-internal, tsv-wasm 7.7x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 14.70   | 14.74   | 14.81   | 14.81   | 14.82   | 14.62   | 14.82   | baseline              | baseline |
| tsv         | 2.51       | 13 | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 36.9x                 | 36.9x    |
| tsv-wasm    | 1.80       | 9  | 0.56    | 0.56    | 0.56    | —       | —       | 0.55    | 0.56    | 26.4x                 | 26.4x    |
| oxfmt       | 1.17       | 8  | 0.86    | 0.86    | 0.86    | —       | —       | 0.85    | 0.87    | 17.1x                 | 17.1x    |
| biome-wasm  | 0.21       | 8  | 4.75    | 4.77    | 4.77    | —       | —       | 4.74    | 4.77    | 3.09x                 | 3.09x    |
| dprint-wasm | 0.30       | 8  | 3.32    | 3.32    | 3.33    | —       | —       | 3.31    | 3.33    | 4.43x                 | 4.43x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.3 MB/s, tsv 46.3 MB/s, tsv-wasm 33.2 MB/s, oxfmt 21.5 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 83.70      | 369  | 11.86    | 12.25    | 12.62    | 12.84    | 12.99    | 11.65    | 13.19    | baseline                     | baseline |
| tsv                | 48.36      | 203  | 20.72    | 20.81    | 22.85    | 23.42    | 23.71    | 20.07    | 23.90    | 0.58x                        | 0.57x    |
| tsv-wasm           | 46.76      | 192  | 21.38    | 21.58    | 23.26    | 23.67    | 30.78    | 21.14    | 31.04    | 0.56x                        | 0.55x    |
| tsv+locations      | 39.80      | 149  | 25.18    | 25.57    | 27.55    | 28.26    | 28.44    | 24.43    | 28.80    | 0.48x                        | 0.47x    |
| tsv-wasm+locations | 38.78      | 135  | 25.79    | 26.71    | 27.93    | 29.33    | 33.75    | 25.54    | 34.47    | 0.46x                        | 0.46x    |
| tsv-internal       | 357.48     | 1628 | 2.80     | 2.80     | 2.81     | 2.83     | 2.91     | 2.78     | 3.08     | 4.27x                        | 4.24x    |
| tsv-wasm-internal  | 241.97     | 1111 | 4.13     | 4.14     | 4.17     | 4.19     | 4.29     | 4.11     | 4.42     | 2.89x                        | 2.87x    |
| postcss            | 80.94      | 372  | 12.28    | 12.57    | 12.79    | 13.18    | 13.37    | 12.13    | 13.76    | 0.97x                        | 0.97x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 32.9 MB/s, tsv 19.0 MB/s, tsv-wasm 18.4 MB/s, tsv+locations 15.6 MB/s, tsv-wasm+locations 15.2 MB/s, tsv-internal 140.5 MB/s, tsv-wasm-internal 95.1 MB/s, postcss 31.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.4x tsv-internal, tsv-wasm 5.2x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.73       | 15  | 576.94   | 580.60   | 584.17   | 587.57   | 595.28   | 564.99   | 597.20   | baseline              | baseline |
| tsv        | 192.69     | 902 | 5.19     | 5.20     | 5.24     | 5.26     | 5.51     | 5.14     | 9.24     | 111.1x                | 111.2x   |
| tsv-wasm   | 136.87     | 595 | 7.31     | 7.32     | 7.38     | 7.42     | 7.75     | 7.27     | 10.11    | 78.9x                 | 79.0x    |
| oxfmt      | 53.06      | 260 | 18.82    | 19.20    | 19.56    | 19.87    | 20.83    | 17.35    | 22.00    | 30.6x                 | 30.7x    |
| biome-wasm | 10.86      | 35  | 92.33    | 104.38   | 108.58   | 109.73   | 110.67   | 91.35    | 110.75   | 6.26x                 | 6.25x    |
| malva-wasm | 21.26      | 89  | 47.04    | 47.12    | 47.67    | 47.83    | 47.99    | 46.91    | 50.96    | 12.3x                 | 12.3x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 68.2 MB/s, tsv-wasm 48.4 MB/s, oxfmt 18.8 MB/s, biome-wasm 3.8 MB/s, malva-wasm 7.5 MB/s

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
| format svelte (923f) | **78.8x** prettier, **79.0x** oxfmt |
| format typescript (2593f) | **36.9x** prettier, **2.15x** oxfmt |
| format css (54f) | **111.1x** prettier, **3.63x** oxfmt |
| parse svelte (929f) | **3.07x** svelte/compiler, **3.90x** rsvelte-parse |
| parse typescript (2593f) | **3.01x** acorn-typescript, **1.41x** oxc-parser, **0.41x** yuku-parser, **1.86x** swc |
| parse css (55f) | **0.58x** svelte/compiler, **0.60x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **55.3x** prettier, **12.3x** biome-wasm |
| format typescript (2593f) | **26.4x** prettier, **8.55x** biome-wasm, **5.97x** dprint-wasm |
| format css (54f) | **78.9x** prettier, **12.6x** biome-wasm, **6.44x** malva-wasm |
| parse svelte (929f) | **2.81x** svelte/compiler |
| parse typescript (2593f) | **2.85x** acorn-typescript, **1.37x** oxc-parser-wasm, **0.31x** yuku-parser-wasm |
| parse css (55f) | **0.56x** svelte/compiler, **0.58x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.32x, wasm 1.29x | typescript: native 1.24x, wasm 1.23x | css: native 1.22x, wasm 1.21x

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
