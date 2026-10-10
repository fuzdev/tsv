# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-10T17:36:41.639Z — tsv 0.6.0 (c93c1649)

**Corpus:** 929 Svelte (2.4 MB), 2596 TypeScript (18.4 MB), 55 CSS (0.4 MB) — 3580 files, 21.2 MB total

**Corpus snapshot:** [fuzdev/corpora@63d1790f2](https://github.com/fuzdev/corpora/tree/63d1790f2473b8aa2eb27c0147dda8e89ee0a5bd) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.5, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.4% apart at the median, 2.1% at the 95th percentile and 4.4% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 2.87       | 39  | 349.33   | 351.31   | 356.42   | 358.81   | 361.28   | 345.05   | 361.96   | baseline                     | baseline |
| tsv                         | 6.81       | 104 | 146.24   | 148.28   | 149.10   | 149.31   | 149.91   | 144.60   | 149.99   | 2.38x                        | 2.39x    |
| tsv-wasm                    | 6.20       | 95  | 161.32   | 162.52   | 163.30   | 163.73   | 164.04   | 159.05   | 164.52   | 2.16x                        | 2.17x    |
| tsv+locations               | 5.22       | 74  | 191.69   | 192.92   | 193.60   | 198.40   | 202.98   | 189.85   | 206.66   | 1.82x                        | 1.82x    |
| tsv-wasm+locations          | 4.85       | 72  | 206.34   | 206.93   | 207.32   | 207.56   | 221.65   | 205.12   | 221.74   | 1.69x                        | 1.69x    |
| tsv-internal                | 60.46      | 868 | 16.53    | 16.59    | 16.66    | 16.78    | 17.04    | 16.38    | 18.04    | 21.1x                        | 21.1x    |
| tsv-wasm-internal           | 41.89      | 559 | 23.90    | 23.97    | 24.14    | 24.30    | 24.51    | 23.64    | 24.61    | 14.6x                        | 14.6x    |
| rsvelte-parse               | 1.71       | 27  | 585.86   | 587.65   | 589.77   | 590.66   | 591.01   | 582.80   | 591.12   | 0.60x                        | 0.60x    |
| rsvelte-parse-skip-expr-loc | 2.61       | 42  | 382.55   | 384.11   | 384.49   | 384.84   | 385.02   | 380.05   | 385.13   | 0.91x                        | 0.91x    |

**Files (intersection):** 929

**Throughput:** svelte/compiler 6.9 MB/s, tsv 16.4 MB/s, tsv-wasm 14.9 MB/s, tsv+locations 12.6 MB/s, tsv-wasm+locations 11.7 MB/s, tsv-internal 145.8 MB/s, tsv-wasm-internal 101.0 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.9x tsv-internal, tsv-wasm 6.8x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.19       | 24  | 5373.43  | 5397.38  | 5427.37  | 5431.30  | 5455.66  | 5326.28  | 5462.79  | baseline              | baseline |
| tsv        | 14.72      | 187 | 68.02    | 68.13    | 68.63    | 68.90    | 69.08    | 67.51    | 69.31    | 79.2x                 | 79.0x    |
| tsv-wasm   | 10.30      | 145 | 97.17    | 97.55    | 97.84    | 98.17    | 98.56    | 96.03    | 99.30    | 55.4x                 | 55.3x    |
| oxfmt      | 0.19       | 22  | 5353.38  | 5380.29  | 5387.35  | 5389.41  | 5391.77  | 5292.28  | 5392.44  | 1.00x                 | 1.00x    |
| biome-wasm | 1.00       | 21  | 998.46   | 1003.51  | 1062.30  | 1089.24  | 1094.10  | 989.33   | 1095.36  | 5.39x                 | 5.38x    |

**Files (intersection):** 923

**Throughput:** prettier 0.4 MB/s, tsv 34.3 MB/s, tsv-wasm 24.0 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.3 MB/s

**Coverage:** prettier 929/929 (100%), tsv 929/929 (100%), tsv-wasm 929/929 (100%), oxfmt 929/929 (100%), biome-wasm 923/929 (99%)

**Omitted from every row's timed set:** 6 of 929 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 868/929 (93%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 929/929 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.72       | 24  | 1.38    | 1.39    | 1.39    | 1.40    | 1.40    | 1.37    | 1.41    | baseline                      | baseline |
| tsv                | 1.07       | 21  | 0.93    | 0.93    | 0.94    | 0.95    | 0.96    | 0.93    | 0.96    | 1.48x                         | 1.48x    |
| tsv-wasm           | 1.01       | 21  | 0.99    | 0.99    | 1.00    | 1.01    | 1.01    | 0.98    | 1.02    | 1.40x                         | 1.40x    |
| tsv+locations      | 0.88       | 24  | 1.13    | 1.15    | 1.15    | 1.15    | 1.15    | 1.12    | 1.15    | 1.22x                         | 1.22x    |
| tsv-wasm+locations | 0.84       | 23  | 1.19    | 1.19    | 1.19    | 1.19    | 1.19    | 1.18    | 1.19    | 1.16x                         | 1.16x    |
| tsv-internal       | 9.87       | 138 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.11    | 13.6x                         | 13.6x    |
| tsv-wasm-internal  | 7.43       | 108 | 0.13    | 0.13    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 10.3x                         | 10.3x    |
| oxc-parser         | 0.73       | 23  | 1.37    | 1.38    | 1.39    | 1.39    | 1.39    | 1.35    | 1.40    | 1.01x                         | 1.01x    |
| oxc-parser-wasm    | 0.70       | 22  | 1.43    | 1.44    | 1.45    | 1.45    | 1.45    | 1.42    | 1.45    | 0.97x                         | 0.97x    |
| yuku-parser        | 2.62       | 41  | 0.38    | 0.39    | 0.39    | 0.39    | 0.39    | 0.37    | 0.39    | 3.62x                         | 3.62x    |
| yuku-parser-wasm   | 3.32       | 51  | 0.30    | 0.30    | 0.31    | 0.31    | 0.31    | 0.29    | 0.31    | 4.60x                         | 4.60x    |
| swc                | 0.57       | 24  | 1.76    | 1.79    | 1.79    | 1.79    | 1.79    | 1.75    | 1.79    | 0.78x                         | 0.79x    |

**Files (intersection):** 2593

**Throughput:** acorn-typescript 13.3 MB/s, tsv 19.8 MB/s, tsv-wasm 18.7 MB/s, tsv+locations 16.3 MB/s, tsv-wasm+locations 15.5 MB/s, tsv-internal 181.8 MB/s, tsv-wasm-internal 137.0 MB/s, oxc-parser 13.4 MB/s, oxc-parser-wasm 12.9 MB/s, yuku-parser 48.3 MB/s, yuku-parser-wasm 61.2 MB/s, swc 10.4 MB/s

**Coverage:** acorn-typescript 2593/2596 (99%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), tsv+locations 2596/2596 (100%), tsv-wasm+locations 2596/2596 (100%), tsv-internal 2596/2596 (100%), tsv-wasm-internal 2596/2596 (100%), oxc-parser 2594/2596 (99%), oxc-parser-wasm 2594/2596 (99%), yuku-parser 2594/2596 (99%), yuku-parser-wasm 2594/2596 (99%), swc 2593/2596 (99%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.2x tsv-internal, tsv-wasm 7.3x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 24 | 14.49   | 14.54   | 14.62   | 14.62   | 14.66   | 14.32   | 14.66   | baseline              | baseline |
| tsv         | 2.51       | 39 | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 36.4x                 | 36.4x    |
| tsv-wasm    | 1.80       | 26 | 0.56    | 0.56    | 0.56    | 0.56    | 0.56    | 0.55    | 0.56    | 26.1x                 | 26.0x    |
| oxfmt       | 1.15       | 24 | 0.86    | 0.89    | 0.89    | 0.89    | 0.90    | 0.84    | 0.90    | 16.7x                 | 16.9x    |
| biome-wasm  | 0.24       | 21 | 4.23    | 4.25    | 4.36    | 4.42    | 4.45    | 4.19    | 4.45    | 3.42x                 | 3.42x    |
| dprint-wasm | 0.30       | 24 | 3.31    | 3.32    | 3.32    | 3.33    | 3.33    | 3.30    | 3.33    | 4.37x                 | 4.38x    |

**Files (intersection):** 2593

**Throughput:** prettier 1.3 MB/s, tsv 46.3 MB/s, tsv-wasm 33.2 MB/s, oxfmt 21.3 MB/s, biome-wasm 4.4 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2596/2596 (100%), tsv 2596/2596 (100%), tsv-wasm 2596/2596 (100%), oxfmt 2594/2596 (99%), biome-wasm 2593/2596 (99%), dprint-wasm 2596/2596 (100%)

**Omitted from every row's timed set:** 3 of 2596 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 95.19      | 1203 | 10.46    | 10.72    | 11.08    | 11.30    | 11.60    | 10.28    | 14.50    | baseline                     | baseline |
| tsv                | 51.44      | 753  | 19.32    | 19.89    | 20.02    | 20.11    | 20.70    | 18.43    | 22.57    | 0.54x                        | 0.54x    |
| tsv-wasm           | 48.47      | 612  | 20.68    | 20.95    | 21.68    | 23.96    | 24.90    | 19.75    | 26.06    | 0.51x                        | 0.51x    |
| tsv+locations      | 42.98      | 625  | 23.35    | 23.47    | 23.60    | 23.77    | 24.27    | 22.27    | 24.90    | 0.45x                        | 0.45x    |
| tsv-wasm+locations | 42.90      | 489  | 23.35    | 23.70    | 25.41    | 25.92    | 26.41    | 22.94    | 27.59    | 0.45x                        | 0.45x    |
| tsv-internal       | 360.22     | 4880 | 2.78     | 2.79     | 2.80     | 2.82     | 2.89     | 2.75     | 7.35     | 3.78x                        | 3.76x    |
| tsv-wasm-internal  | 241.90     | 3545 | 4.12     | 4.16     | 4.21     | 4.26     | 4.33     | 4.04     | 8.13     | 2.54x                        | 2.54x    |
| postcss            | 83.04      | 964  | 12.02    | 12.31    | 12.71    | 12.93    | 13.11    | 11.87    | 13.70    | 0.87x                        | 0.87x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 37.4 MB/s, tsv 20.2 MB/s, tsv-wasm 19.0 MB/s, tsv+locations 16.9 MB/s, tsv-wasm+locations 16.9 MB/s, tsv-internal 141.6 MB/s, tsv-wasm-internal 95.1 MB/s, postcss 32.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.0x tsv-internal, tsv-wasm 5.0x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.75       | 26   | 572.01   | 577.00   | 584.44   | 587.30   | 601.64   | 556.93   | 606.34   | baseline              | baseline |
| tsv        | 193.95     | 2589 | 5.16     | 5.18     | 5.25     | 5.49     | 5.60     | 5.10     | 9.18     | 111.0x                | 111.0x   |
| tsv-wasm   | 136.63     | 1831 | 7.32     | 7.37     | 7.47     | 7.65     | 7.78     | 7.20     | 8.46     | 78.2x                 | 78.1x    |
| oxfmt      | 53.64      | 782  | 18.63    | 18.96    | 19.25    | 19.48    | 20.20    | 17.27    | 21.44    | 30.7x                 | 30.7x    |
| biome-wasm | 11.54      | 118  | 86.87    | 92.14    | 94.10    | 94.88    | 97.66    | 85.26    | 99.88    | 6.60x                 | 6.58x    |
| malva-wasm | 21.21      | 284  | 47.13    | 47.33    | 47.63    | 47.81    | 48.09    | 46.85    | 51.51    | 12.1x                 | 12.1x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 68.6 MB/s, tsv-wasm 48.4 MB/s, oxfmt 19.0 MB/s, biome-wasm 4.1 MB/s, malva-wasm 7.5 MB/s

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
| format svelte (923f) | **79.2x** prettier, **78.8x** oxfmt |
| format typescript (2593f) | **36.4x** prettier, **2.17x** oxfmt |
| format css (54f) | **111.0x** prettier, **3.62x** oxfmt |
| parse svelte (929f) | **2.38x** svelte/compiler, **3.99x** rsvelte-parse |
| parse typescript (2593f) | **1.48x** acorn-typescript, **1.47x** oxc-parser, **0.41x** yuku-parser, **1.90x** swc |
| parse css (55f) | **0.54x** svelte/compiler, **0.62x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (923f) | **55.4x** prettier, **10.3x** biome-wasm |
| format typescript (2593f) | **26.1x** prettier, **7.61x** biome-wasm, **5.96x** dprint-wasm |
| format css (54f) | **78.2x** prettier, **11.8x** biome-wasm, **6.44x** malva-wasm |
| parse svelte (929f) | **2.16x** svelte/compiler |
| parse typescript (2593f) | **1.40x** acorn-typescript, **1.45x** oxc-parser-wasm, **0.30x** yuku-parser-wasm |
| parse css (55f) | **0.51x** svelte/compiler, **0.58x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.31x, wasm 1.28x | typescript: native 1.22x, wasm 1.20x | css: native 1.20x, wasm 1.13x

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
