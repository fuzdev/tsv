# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-18T06:35:57.760Z — tsv 0.7.0 (7103b4aa)

**Corpus:** 1913 Svelte (4.9 MB), 5700 TypeScript (34.1 MB), 73 CSS (0.5 MB) — 7686 files, 39.5 MB total

**Corpus snapshot:** [fuzdev/corpora@976ba903d](https://github.com/fuzdev/corpora/tree/976ba903d97b2ceb236df9a511dcdea1591843e4) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 57% third-party by bytes (1091 files, 2.8 MB third-party · 822 files, 2.1 MB the author's own); TypeScript 56% third-party by bytes (3895 files, 19.3 MB third-party · 1805 files, 14.9 MB the author's own); CSS 47% third-party by bytes (32 files, 0.2 MB third-party · 41 files, 0.3 MB the author's own)

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (532), ../corpora/collections/layercake/src (246), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (149), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/core/src (1088), ../corpora/collections/storybook/code/lib/cli-storybook/src (111), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (75), ../corpora/collections/storybook/code/lib/csf-plugin/src (4), ../corpora/collections/storybook/code/lib/eslint-plugin/src (47), ../corpora/collections/storybook/code/lib/react-dom-shim/src (1), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (25)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.5, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 2.7% at the 95th percentile and 3.8% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.25       | 24  | 804.95   | 810.95   | 816.28   | 817.08   | 819.77   | 776.51   | 820.56   | baseline                     | baseline |
| tsv                         | 4.90       | 75  | 204.06   | 205.69   | 206.25   | 207.31   | 209.21   | 201.01   | 209.46   | 3.93x                        | 3.94x    |
| tsv-wasm                    | 4.37       | 65  | 228.17   | 230.58   | 232.59   | 233.30   | 234.03   | 225.11   | 234.15   | 3.51x                        | 3.53x    |
| tsv+locations               | 3.40       | 52  | 293.83   | 295.49   | 297.03   | 298.25   | 299.32   | 288.30   | 299.51   | 2.73x                        | 2.74x    |
| tsv-wasm+locations          | 3.11       | 46  | 321.30   | 322.66   | 323.57   | 325.54   | 328.81   | 317.87   | 331.14   | 2.50x                        | 2.51x    |
| tsv-internal                | 34.24      | 501 | 29.17    | 29.35    | 29.54    | 29.66    | 29.99    | 28.83    | 30.25    | 27.5x                        | 27.6x    |
| tsv-wasm-internal           | 20.73      | 284 | 48.26    | 48.53    | 48.82    | 49.12    | 49.62    | 47.58    | 50.36    | 16.6x                        | 16.7x    |
| rsvelte-parse               | 1.02       | 24  | 981.35   | 982.88   | 984.33   | 985.82   | 986.89   | 972.45   | 987.14   | 0.82x                        | 0.82x    |
| rsvelte-parse-skip-expr-loc | 1.58       | 23  | 631.72   | 634.35   | 635.32   | 635.95   | 638.31   | 624.46   | 638.99   | 1.27x                        | 1.27x    |

**Files (intersection):** 1913

**Throughput:** svelte/compiler 6.1 MB/s, tsv 23.8 MB/s, tsv-wasm 21.3 MB/s, tsv+locations 16.6 MB/s, tsv-wasm+locations 15.1 MB/s, tsv-internal 166.5 MB/s, tsv-wasm-internal 100.8 MB/s, rsvelte-parse 5.0 MB/s, rsvelte-parse-skip-expr-loc 7.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.0x tsv-internal, tsv-wasm 4.7x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.11       | 23  | 9.27    | 9.32    | 9.38    | 9.41    | 9.50    | 9.05    | 9.53    | baseline              | baseline |
| tsv        | 7.53       | 114 | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 69.7x                 | 69.9x    |
| tsv-wasm   | 5.19       | 76  | 0.19    | 0.19    | 0.19    | 0.19    | 0.20    | 0.19    | 0.20    | 48.0x                 | 48.1x    |
| oxfmt      | 0.11       | 24  | 9.51    | 9.65    | 9.68    | 9.69    | 9.70    | 9.24    | 9.70    | 0.97x                 | 0.97x    |
| biome-wasm | 0.57       | 24  | 1.76    | 1.78    | 1.79    | 1.80    | 1.81    | 1.71    | 1.81    | 5.26x                 | 5.27x    |

**Files (intersection):** 1906

**Throughput:** prettier 0.5 MB/s, tsv 36.0 MB/s, tsv-wasm 24.8 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

**Coverage:** prettier 1913/1913 (100%), tsv 1913/1913 (100%), tsv-wasm 1913/1913 (100%), oxfmt 1913/1913 (100%), biome-wasm 1906/1913 (99%)

**Omitted from every row's timed set:** 7 of 1913 files, 1.8% of the group's bytes (0.4% of its files) — by row: biome-wasm 7 (7 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 1795/1913 (93%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 1913/1913 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.41       | 20  | 2.44    | 2.45    | 2.47    | 2.49    | 2.52    | 2.37    | 2.52    | baseline                      | baseline |
| tsv                | 0.90       | 23  | 1.11    | 1.12    | 1.12    | 1.12    | 1.12    | 1.09    | 1.12    | 2.20x                         | 2.21x    |
| tsv-wasm           | 0.86       | 24  | 1.17    | 1.18    | 1.18    | 1.19    | 1.19    | 1.14    | 1.19    | 2.09x                         | 2.09x    |
| tsv+locations      | 0.66       | 24  | 1.51    | 1.53    | 1.54    | 1.54    | 1.54    | 1.46    | 1.54    | 1.62x                         | 1.62x    |
| tsv-wasm+locations | 0.63       | 24  | 1.60    | 1.61    | 1.61    | 1.61    | 1.62    | 1.55    | 1.62    | 1.53x                         | 1.53x    |
| tsv-internal       | 6.67       | 101 | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 16.3x                         | 16.3x    |
| tsv-wasm-internal  | 4.14       | 63  | 0.24    | 0.24    | 0.24    | 0.24    | 0.24    | 0.24    | 0.24    | 10.1x                         | 10.1x    |
| oxc-parser         | 0.64       | 24  | 1.56    | 1.57    | 1.59    | 1.60    | 1.60    | 1.50    | 1.60    | 1.56x                         | 1.57x    |
| oxc-parser-wasm    | 0.51       | 24  | 1.96    | 1.99    | 2.00    | 2.01    | 2.02    | 1.87    | 2.02    | 1.25x                         | 1.24x    |
| yuku-parser        | 1.70       | 24  | 0.59    | 0.59    | 0.59    | 0.59    | 0.60    | 0.58    | 0.60    | 4.15x                         | 4.14x    |
| yuku-parser-wasm   | 2.23       | 36  | 0.45    | 0.45    | 0.46    | 0.46    | 0.46    | 0.44    | 0.46    | 5.44x                         | 5.45x    |
| swc                | 0.41       | 24  | 2.42    | 2.42    | 2.42    | 2.42    | 2.43    | 2.41    | 2.43    | 1.01x                         | 1.01x    |

**Files (intersection):** 5697

**Throughput:** acorn-typescript 14.0 MB/s, tsv 30.8 MB/s, tsv-wasm 29.3 MB/s, tsv+locations 22.7 MB/s, tsv-wasm+locations 21.5 MB/s, tsv-internal 227.5 MB/s, tsv-wasm-internal 141.2 MB/s, oxc-parser 21.9 MB/s, oxc-parser-wasm 17.5 MB/s, yuku-parser 58.1 MB/s, yuku-parser-wasm 76.1 MB/s, swc 14.1 MB/s

**Coverage:** acorn-typescript 5697/5700 (99%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), tsv+locations 5700/5700 (100%), tsv-wasm+locations 5700/5700 (100%), tsv-internal 5700/5700 (100%), tsv-wasm-internal 5700/5700 (100%), oxc-parser 5698/5700 (99%), oxc-parser-wasm 5698/5700 (99%), yuku-parser 5698/5700 (99%), yuku-parser-wasm 5698/5700 (99%), swc 5697/5700 (99%)

**Omitted from every row's timed set:** 3 of 5700 files, 0.0% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.4x tsv-internal, tsv-wasm 4.8x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 24 | 25.88   | 26.01   | 26.14   | 26.24   | 26.35   | 25.46   | 26.38   | baseline              | baseline |
| tsv         | 1.41       | 24 | 0.71    | 0.71    | 0.71    | 0.71    | 0.71    | 0.71    | 0.71    | 36.4x                 | 36.4x    |
| tsv-wasm    | 0.97       | 24 | 1.03    | 1.03    | 1.04    | 1.04    | 1.04    | 1.02    | 1.04    | 25.1x                 | 25.1x    |
| oxfmt       | 0.63       | 24 | 1.59    | 1.60    | 1.61    | 1.62    | 1.62    | 1.55    | 1.62    | 16.3x                 | 16.3x    |
| biome-wasm  | 0.14       | 21 | 7.29    | 7.32    | 7.34    | 7.34    | 7.51    | 7.20    | 7.56    | 3.55x                 | 3.55x    |
| dprint-wasm | 0.17       | 16 | 5.81    | 5.85    | 5.86    | 5.86    | 5.86    | 5.81    | 5.86    | 4.45x                 | 4.45x    |

**Files (intersection):** 5697

**Throughput:** prettier 1.3 MB/s, tsv 48.0 MB/s, tsv-wasm 33.1 MB/s, oxfmt 21.5 MB/s, biome-wasm 4.7 MB/s, dprint-wasm 5.9 MB/s

**Coverage:** prettier 5700/5700 (100%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), oxfmt 5698/5700 (99%), biome-wasm 5697/5700 (99%), dprint-wasm 5700/5700 (100%)

**Omitted from every row's timed set:** 3 of 5700 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 42.26      | 635  | 23.78    | 24.51    | 24.96    | 25.17    | 25.78    | 21.04    | 26.49    | baseline                     | baseline |
| tsv                | 50.48      | 758  | 19.38    | 20.46    | 20.78    | 21.37    | 22.53    | 18.40    | 24.31    | 1.19x                        | 1.23x    |
| tsv-wasm           | 60.08      | 875  | 16.56    | 16.86    | 17.39    | 17.65    | 18.36    | 15.93    | 21.89    | 1.42x                        | 1.44x    |
| tsv+locations      | 39.24      | 584  | 25.34    | 26.16    | 27.37    | 27.97    | 28.83    | 23.17    | 29.51    | 0.93x                        | 0.94x    |
| tsv-wasm+locations | 46.03      | 618  | 21.54    | 22.19    | 22.77    | 23.03    | 23.69    | 20.94    | 24.69    | 1.09x                        | 1.10x    |
| tsv-internal       | 363.10     | 5180 | 2.75     | 2.76     | 2.77     | 2.79     | 2.87     | 2.72     | 6.76     | 8.59x                        | 8.63x    |
| tsv-wasm-internal  | 246.83     | 3423 | 4.08     | 4.09     | 4.12     | 4.14     | 4.23     | 3.95     | 4.97     | 5.84x                        | 5.83x    |
| postcss            | 46.75      | 687  | 21.43    | 21.91    | 22.38    | 22.71    | 23.60    | 18.70    | 24.72    | 1.11x                        | 1.11x    |

**Files (intersection):** 73

**Throughput:** svelte/compiler 21.3 MB/s, tsv 25.4 MB/s, tsv-wasm 30.3 MB/s, tsv+locations 19.8 MB/s, tsv-wasm+locations 23.2 MB/s, tsv-internal 182.9 MB/s, tsv-wasm-internal 124.4 MB/s, postcss 23.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.2x tsv-internal, tsv-wasm 4.1x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.69       | 27   | 592.52   | 598.36   | 602.76   | 605.94   | 607.15   | 569.23   | 607.24   | baseline              | baseline |
| tsv        | 178.97     | 2110 | 5.59     | 5.65     | 5.87     | 6.10     | 6.30     | 5.54     | 8.70     | 106.0x                | 106.1x   |
| tsv-wasm   | 131.19     | 1762 | 7.63     | 7.66     | 7.73     | 7.89     | 8.02     | 7.55     | 11.64    | 77.7x                 | 77.7x    |
| oxfmt      | 43.94      | 654  | 22.75    | 23.11    | 23.45    | 23.75    | 24.40    | 21.05    | 25.37    | 26.0x                 | 26.0x    |
| biome-wasm | 9.93       | 137  | 99.61    | 103.64   | 104.57   | 104.97   | 106.83   | 96.88    | 107.71   | 5.88x                 | 5.95x    |
| malva-wasm | 14.32      | 203  | 69.51    | 70.75    | 70.96    | 71.20    | 71.78    | 68.94    | 73.06    | 8.48x                 | 8.52x    |

**Files (intersection):** 71

**Throughput:** prettier 0.8 MB/s, tsv 82.9 MB/s, tsv-wasm 60.8 MB/s, oxfmt 20.4 MB/s, biome-wasm 4.6 MB/s, malva-wasm 6.6 MB/s

**Coverage:** prettier 73/73 (100%), tsv 73/73 (100%), tsv-wasm 73/73 (100%), oxfmt 73/73 (100%), biome-wasm 71/73 (97%), malva-wasm 73/73 (100%)

**Omitted from every row's timed set:** 2 of 73 files, 8.0% of the group's bytes (2.7% of its files) — by row: biome-wasm 2 (2 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.5 MB | 961.7 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.0 MB | 404.8 KB | 0.4x | 0.4x |
| tsv-wasm | 2.8 MB | 1.1 MB | — | — |
| biome (wasm) | 46.9 MB | 11.9 MB | 16.9x | 11.1x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.4x |
| yuku-parser (wasm) | 698.9 KB | 202.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| markup_fmt (wasm) | 2.0 MB | 557.3 KB | 0.7x | 0.5x |
| tsv (ffi) | 3.6 MB | 1.7 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.3 MB | 1.6 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 711.3 KB | 0.4x | 0.4x |
| tsv (napi) | 4.1 MB | 1.9 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.9 MB | 4.5 MB | 2.7x | 2.4x |
| oxc-parser (napi) | 2.1 MB | 880.8 KB | 0.5x | 0.5x |
| oxfmt (napi) | 8.8 MB | 3.7 MB | 2.2x | 1.9x |
| yuku-parser (napi) | 681.1 KB | 286.0 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 2.2x | 1.9x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 4.4x | 4.0x |
| swc (napi) | 10.5 MB | 10.2 MB | 2.6x | 5.4x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1906f) | **69.7x** prettier, **71.7x** oxfmt |
| format typescript (5697f) | **36.4x** prettier, **2.24x** oxfmt |
| format css (71f) | **106.0x** prettier, **4.07x** oxfmt |
| parse svelte (1913f) | **3.93x** svelte/compiler, **4.80x** rsvelte-parse |
| parse typescript (5697f) | **2.20x** acorn-typescript, **1.41x** oxc-parser, **0.53x** yuku-parser, **2.18x** swc |
| parse css (73f) | **1.19x** svelte/compiler, **1.08x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1906f) | **48.0x** prettier, **9.13x** biome-wasm |
| format typescript (5697f) | **25.1x** prettier, **7.06x** biome-wasm, **5.63x** dprint-wasm |
| format css (71f) | **77.7x** prettier, **13.2x** biome-wasm, **9.16x** malva-wasm |
| parse svelte (1913f) | **3.51x** svelte/compiler |
| parse typescript (5697f) | **2.09x** acorn-typescript, **1.67x** oxc-parser-wasm, **0.38x** yuku-parser-wasm |
| parse css (73f) | **1.42x** svelte/compiler, **1.29x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.44x, wasm 1.40x | typescript: native 1.36x, wasm 1.36x | css: native 1.29x, wasm 1.31x

## Skipped Files

130 files skipped, 142 unique file+error combinations — Svelte 124, TypeScript 4, CSS 2 files.

**Per-benchmark skip counts:**
- format/svelte: markup-fmt-wasm: 118
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
