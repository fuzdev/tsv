# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.4

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-11-02T06:41:43.538Z — tsv 0.8.0 (83aa5582)

**Corpus:** 1898 Svelte (4.8 MB), 5691 TypeScript (34.1 MB), 71 CSS (0.5 MB) — 7660 files, 39.4 MB total

**Corpus snapshot:** [fuzdev/corpora@678f4942f](https://github.com/fuzdev/corpora/tree/678f4942f92ca0fab49a73db37f61546f34b470c) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 57% third-party by bytes (1091 files, 2.8 MB third-party · 807 files, 2.1 MB the author's own); TypeScript 57% third-party by bytes (3895 files, 19.3 MB third-party · 1796 files, 14.8 MB the author's own); CSS 48% third-party by bytes (32 files, 0.2 MB third-party · 39 files, 0.3 MB the author's own)

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (532), ../corpora/collections/layercake/src (246), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (149), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/core/src (1088), ../corpora/collections/storybook/code/lib/cli-storybook/src (111), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (75), ../corpora/collections/storybook/code/lib/csf-plugin/src (4), ../corpora/collections/storybook/code/lib/eslint-plugin/src (47), ../corpora/collections/storybook/code/lib/react-dom-shim/src (1), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (24)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.6% apart at the median, 3.3% at the 95th percentile and 5.9% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.26       | 24  | 790.76   | 800.72   | 807.15   | 811.27   | 818.00   | 765.67   | 819.83   | baseline                     | baseline |
| tsv                         | 5.94       | 90  | 168.46   | 170.02   | 172.05   | 172.47   | 173.13   | 163.54   | 173.50   | 4.70x                        | 4.69x    |
| tsv-wasm                    | 4.99       | 75  | 200.14   | 201.17   | 201.79   | 202.29   | 202.76   | 197.83   | 203.07   | 3.95x                        | 3.95x    |
| tsv+locations               | 3.88       | 60  | 257.99   | 260.25   | 262.36   | 263.01   | 264.84   | 250.05   | 265.91   | 3.07x                        | 3.07x    |
| tsv-wasm+locations          | 3.42       | 53  | 292.31   | 293.68   | 295.35   | 296.93   | 299.54   | 289.35   | 301.03   | 2.71x                        | 2.71x    |
| tsv-internal                | 32.40      | 423 | 30.86    | 31.11    | 31.37    | 31.54    | 32.03    | 30.54    | 32.80    | 25.6x                        | 25.6x    |
| tsv-wasm-internal           | 20.03      | 265 | 49.91    | 50.12    | 50.61    | 50.79    | 51.40    | 49.58    | 51.55    | 15.9x                        | 15.8x    |
| rsvelte-parse               | 1.10       | 23  | 907.32   | 908.69   | 911.96   | 912.44   | 914.57   | 902.16   | 915.20   | 0.87x                        | 0.87x    |
| rsvelte-parse-skip-expr-loc | 1.72       | 27  | 579.96   | 581.57   | 582.86   | 584.03   | 584.15   | 575.91   | 584.17   | 1.36x                        | 1.36x    |

**Files (intersection):** 1898

**Throughput:** svelte/compiler 6.1 MB/s, tsv 28.7 MB/s, tsv-wasm 24.1 MB/s, tsv+locations 18.7 MB/s, tsv-wasm+locations 16.5 MB/s, tsv-internal 156.6 MB/s, tsv-wasm-internal 96.8 MB/s, rsvelte-parse 5.3 MB/s, rsvelte-parse-skip-expr-loc 8.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 5.5x tsv-internal, tsv-wasm 4.0x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.11       | 24  | 9.26    | 9.36    | 9.44    | 9.56    | 9.61    | 9.08    | 9.62    | baseline              | baseline |
| tsv        | 7.34       | 110 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 68.2x                 | 68.0x    |
| tsv-wasm   | 5.12       | 75  | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.19    | 0.20    | 47.5x                 | 47.4x    |
| oxfmt      | 0.10       | 24  | 9.56    | 9.70    | 9.76    | 9.77    | 9.85    | 9.39    | 9.87    | 0.97x                 | 0.97x    |
| biome-wasm | 0.53       | 24  | 1.88    | 1.90    | 1.91    | 1.92    | 1.92    | 1.80    | 1.92    | 4.96x                 | 4.93x    |

**Files (intersection):** 1891

**Throughput:** prettier 0.5 MB/s, tsv 34.9 MB/s, tsv-wasm 24.3 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 1898/1898 (100%), tsv 1898/1898 (100%), tsv-wasm 1898/1898 (100%), oxfmt 1898/1898 (100%), biome-wasm 1891/1898 (99%)

**Omitted from every row's timed set:** 7 of 1898 files, 1.8% of the group's bytes (0.4% of its files) — by row: biome-wasm 7 (7 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 1828/1898 (96%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 1898/1898 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.43       | 24 | 2.33    | 2.36    | 2.37    | 2.38    | 2.38    | 2.28    | 2.39    | baseline                      | baseline |
| tsv                | 1.24       | 22 | 0.80    | 0.81    | 0.81    | 0.81    | 0.81    | 0.79    | 0.82    | 2.91x                         | 2.90x    |
| tsv-wasm           | 1.03       | 23 | 0.97    | 0.98    | 0.99    | 0.99    | 0.99    | 0.96    | 0.99    | 2.39x                         | 2.40x    |
| tsv+locations      | 0.85       | 23 | 1.18    | 1.19    | 1.19    | 1.19    | 1.19    | 1.16    | 1.19    | 1.98x                         | 1.97x    |
| tsv-wasm+locations | 0.73       | 23 | 1.37    | 1.37    | 1.37    | 1.37    | 1.38    | 1.35    | 1.38    | 1.71x                         | 1.71x    |
| tsv-internal       | 6.19       | 94 | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 0.16    | 14.5x                         | 14.5x    |
| tsv-wasm-internal  | 3.94       | 60 | 0.25    | 0.26    | 0.26    | 0.26    | 0.26    | 0.25    | 0.26    | 9.19x                         | 9.17x    |
| oxc-parser         | 0.85       | 24 | 1.17    | 1.18    | 1.19    | 1.20    | 1.20    | 1.15    | 1.20    | 1.99x                         | 1.99x    |
| oxc-parser-wasm    | 0.58       | 24 | 1.73    | 1.78    | 1.79    | 1.82    | 1.83    | 1.67    | 1.83    | 1.34x                         | 1.35x    |
| yuku-parser        | 1.75       | 26 | 0.57    | 0.58    | 0.59    | 0.59    | 0.62    | 0.56    | 0.62    | 4.09x                         | 4.13x    |
| yuku-parser-wasm   | 2.27       | 33 | 0.44    | 0.45    | 0.45    | 0.46    | 0.47    | 0.43    | 0.47    | 5.30x                         | 5.32x    |
| swc                | 0.52       | 24 | 1.92    | 1.93    | 1.94    | 1.94    | 1.94    | 1.91    | 1.94    | 1.21x                         | 1.21x    |

**Files (intersection):** 5688

**Throughput:** acorn-typescript 14.6 MB/s, tsv 42.4 MB/s, tsv-wasm 34.9 MB/s, tsv+locations 28.9 MB/s, tsv-wasm+locations 25.0 MB/s, tsv-internal 211.0 MB/s, tsv-wasm-internal 134.1 MB/s, oxc-parser 29.1 MB/s, oxc-parser-wasm 19.6 MB/s, yuku-parser 59.7 MB/s, yuku-parser-wasm 77.4 MB/s, swc 17.7 MB/s

**Coverage:** acorn-typescript 5688/5691 (99%), tsv 5691/5691 (100%), tsv-wasm 5691/5691 (100%), tsv+locations 5691/5691 (100%), tsv-wasm+locations 5691/5691 (100%), tsv-internal 5691/5691 (100%), tsv-wasm-internal 5691/5691 (100%), oxc-parser 5689/5691 (99%), oxc-parser-wasm 5689/5691 (99%), yuku-parser 5689/5691 (99%), yuku-parser-wasm 5689/5691 (99%), swc 5688/5691 (99%)

**Omitted from every row's timed set:** 3 of 5691 files, 0.0% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 5.0x tsv-internal, tsv-wasm 3.8x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 24 | 25.69   | 26.45   | 26.63   | 26.72   | 26.77   | 25.17   | 26.78   | baseline              | baseline |
| tsv         | 1.36       | 22 | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 0.73    | 0.74    | 35.2x                 | 34.9x    |
| tsv-wasm    | 0.93       | 23 | 1.07    | 1.08    | 1.08    | 1.08    | 1.08    | 1.07    | 1.08    | 24.1x                 | 23.9x    |
| oxfmt       | 0.74       | 24 | 1.34    | 1.35    | 1.39    | 1.40    | 1.40    | 1.32    | 1.40    | 19.2x                 | 19.2x    |
| biome-wasm  | 0.13       | 24 | 7.78    | 7.81    | 7.83    | 7.83    | 7.85    | 7.71    | 7.86    | 3.33x                 | 3.30x    |
| dprint-wasm | 0.17       | 24 | 5.84    | 5.86    | 5.88    | 5.89    | 5.89    | 5.81    | 5.89    | 4.43x                 | 4.40x    |

**Files (intersection):** 5688

**Throughput:** prettier 1.3 MB/s, tsv 46.4 MB/s, tsv-wasm 31.7 MB/s, oxfmt 25.4 MB/s, biome-wasm 4.4 MB/s, dprint-wasm 5.8 MB/s

**Coverage:** prettier 5691/5691 (100%), tsv 5691/5691 (100%), tsv-wasm 5691/5691 (100%), oxfmt 5689/5691 (99%), biome-wasm 5688/5691 (99%), dprint-wasm 5691/5691 (100%)

**Omitted from every row's timed set:** 3 of 5691 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 40.83      | 607  | 24.56    | 24.92    | 25.27    | 25.50    | 26.28    | 22.37    | 27.07    | baseline                     | baseline |
| tsv                | 79.00      | 1186 | 12.65    | 13.01    | 13.46    | 13.61    | 13.87    | 11.56    | 14.32    | 1.93x                        | 1.94x    |
| tsv-wasm           | 73.17      | 1093 | 13.56    | 13.97    | 14.51    | 14.89    | 15.42    | 12.46    | 16.29    | 1.79x                        | 1.81x    |
| tsv+locations      | 56.00      | 814  | 17.86    | 17.98    | 18.10    | 18.22    | 18.62    | 17.50    | 21.51    | 1.37x                        | 1.37x    |
| tsv-wasm+locations | 54.09      | 739  | 18.54    | 18.86    | 19.37    | 19.64    | 20.21    | 17.68    | 21.41    | 1.32x                        | 1.32x    |
| tsv-internal       | 354.21     | 4877 | 2.82     | 2.83     | 2.84     | 2.86     | 2.96     | 2.80     | 3.39     | 8.67x                        | 8.70x    |
| tsv-wasm-internal  | 248.40     | 3359 | 4.03     | 4.06     | 4.07     | 4.09     | 4.21     | 3.97     | 8.11     | 6.08x                        | 6.10x    |
| postcss            | 47.32      | 709  | 21.17    | 21.69    | 22.17    | 22.41    | 23.14    | 18.39    | 24.88    | 1.16x                        | 1.16x    |

**Files (intersection):** 71

**Throughput:** svelte/compiler 20.5 MB/s, tsv 39.6 MB/s, tsv-wasm 36.7 MB/s, tsv+locations 28.1 MB/s, tsv-wasm+locations 27.1 MB/s, tsv-internal 177.5 MB/s, tsv-wasm-internal 124.5 MB/s, postcss 23.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 4.5x tsv-internal, tsv-wasm 3.4x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.75       | 26   | 571.14   | 575.56   | 582.50   | 585.30   | 606.66   | 559.50   | 613.78   | baseline              | baseline |
| tsv        | 185.24     | 2171 | 5.40     | 5.46     | 5.65     | 5.69     | 5.84     | 5.35     | 6.70     | 105.8x                | 105.8x   |
| tsv-wasm   | 133.21     | 1807 | 7.52     | 7.57     | 7.65     | 7.78     | 7.95     | 7.37     | 8.62     | 76.1x                 | 76.0x    |
| oxfmt      | 46.05      | 678  | 21.68    | 22.07    | 22.47    | 22.75    | 23.79    | 20.38    | 25.11    | 26.3x                 | 26.3x    |
| biome-wasm | 11.73      | 165  | 84.45    | 86.70    | 88.94    | 89.42    | 90.49    | 81.76    | 91.31    | 6.70x                 | 6.76x    |
| malva-wasm | 14.22      | 190  | 70.19    | 71.97    | 72.21    | 72.59    | 73.15    | 68.36    | 74.59    | 8.12x                 | 8.14x    |

**Files (intersection):** 69

**Throughput:** prettier 0.8 MB/s, tsv 85.4 MB/s, tsv-wasm 61.4 MB/s, oxfmt 21.2 MB/s, biome-wasm 5.4 MB/s, malva-wasm 6.6 MB/s

**Coverage:** prettier 71/71 (100%), tsv 71/71 (100%), tsv-wasm 71/71 (100%), oxfmt 71/71 (100%), biome-wasm 69/71 (97%), malva-wasm 71/71 (100%)

**Omitted from every row's timed set:** 2 of 71 files, 8.0% of the group's bytes (2.8% of its files) — by row: biome-wasm 2 (2 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.7 MB | 1.1 MB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.1 MB | 424.9 KB | 0.4x | 0.4x |
| tsv-wasm | 3.0 MB | 1.2 MB | — | — |
| biome (wasm) | 64.1 MB | 16.7 MB | 21.1x | 14.3x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.4x | 1.0x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.4x |
| yuku-parser (wasm) | 718.6 KB | 209.4 KB | 0.2x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| markup_fmt (wasm) | 2.0 MB | 558.7 KB | 0.7x | 0.5x |
| tsv (ffi) | 3.9 MB | 1.8 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.6 MB | 1.7 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 739.0 KB | 0.4x | 0.4x |
| tsv (napi) | 4.3 MB | 2.0 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.7 MB | 2.6x | 2.3x |
| oxc-parser (napi) | 2.2 MB | 906.0 KB | 0.5x | 0.4x |
| oxfmt (napi) | 9.1 MB | 3.8 MB | 2.1x | 1.8x |
| yuku-parser (napi) | 692.2 KB | 287.7 KB | 0.2x | 0.1x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 2.1x | 1.8x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 4.1x | 3.7x |
| swc (napi) | 10.7 MB | 10.3 MB | 2.5x | 5.0x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.7x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.7x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1891f) | **68.2x** prettier, **70.4x** oxfmt |
| format typescript (5688f) | **35.2x** prettier, **1.83x** oxfmt |
| format css (69f) | **105.8x** prettier, **4.02x** oxfmt |
| parse svelte (1898f) | **4.70x** svelte/compiler, **5.39x** rsvelte-parse |
| parse typescript (5688f) | **2.91x** acorn-typescript, **1.46x** oxc-parser, **0.71x** yuku-parser, **2.40x** swc |
| parse css (71f) | **1.93x** svelte/compiler, **1.67x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1891f) | **47.5x** prettier, **9.57x** biome-wasm |
| format typescript (5688f) | **24.1x** prettier, **7.25x** biome-wasm, **5.44x** dprint-wasm |
| format css (69f) | **76.1x** prettier, **11.4x** biome-wasm, **9.37x** malva-wasm |
| parse svelte (1898f) | **3.95x** svelte/compiler |
| parse typescript (5688f) | **2.39x** acorn-typescript, **1.78x** oxc-parser-wasm, **0.45x** yuku-parser-wasm |
| parse css (71f) | **1.79x** svelte/compiler, **1.55x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.53x, wasm 1.46x | typescript: native 1.47x, wasm 1.40x | css: native 1.41x, wasm 1.35x

## Unstable Rows

2 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse/css/tsv-wasm+locations | 3.2% | 3.3% | -2.7% | 5.9% | 739/809 |
| format/typescript/oxfmt | 1.9% | 1.9% | +5.8% | 1.6% | 24/24 |

## Skipped Files

82 files skipped, 94 unique file+error combinations — Svelte 76, TypeScript 4, CSS 2 files.

**Per-benchmark skip counts:**
- format/svelte: markup-fmt-wasm: 70
- format/svelte: biome-wasm: 7
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- format/typescript: biome-wasm: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2
- format/css: biome-wasm: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
