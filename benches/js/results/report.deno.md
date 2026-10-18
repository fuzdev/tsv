# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.7

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-18T04:22:30.162Z — tsv 0.7.0 (7103b4aa)

**Corpus:** 1913 Svelte (4.9 MB), 5700 TypeScript (34.1 MB), 73 CSS (0.5 MB) — 7686 files, 39.5 MB total

**Corpus snapshot:** [fuzdev/corpora@976ba903d](https://github.com/fuzdev/corpora/tree/976ba903d97b2ceb236df9a511dcdea1591843e4) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 57% third-party by bytes (1091 files, 2.8 MB third-party · 822 files, 2.1 MB the author's own); TypeScript 56% third-party by bytes (3895 files, 19.3 MB third-party · 1805 files, 14.9 MB the author's own); CSS 47% third-party by bytes (32 files, 0.2 MB third-party · 41 files, 0.3 MB the author's own)

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (532), ../corpora/collections/layercake/src (246), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (149), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/core/src (1088), ../corpora/collections/storybook/code/lib/cli-storybook/src (111), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (75), ../corpora/collections/storybook/code/lib/csf-plugin/src (4), ../corpora/collections/storybook/code/lib/eslint-plugin/src (47), ../corpora/collections/storybook/code/lib/react-dom-shim/src (1), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (25)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.5, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 2.3% at the 95th percentile and 4.3% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.38       | 23  | 723.92   | 729.23   | 733.67   | 733.84   | 734.78   | 710.49   | 735.05   | baseline                     | baseline |
| tsv                         | 3.71       | 56  | 269.41   | 271.02   | 271.86   | 272.29   | 273.80   | 267.51   | 274.43   | 2.69x                        | 2.69x    |
| tsv-wasm                    | 3.00       | 45  | 333.76   | 334.18   | 334.59   | 334.66   | 335.24   | 332.05   | 335.24   | 2.17x                        | 2.17x    |
| tsv+locations               | 2.63       | 39  | 379.63   | 380.30   | 380.95   | 381.13   | 381.96   | 377.50   | 382.16   | 1.91x                        | 1.91x    |
| tsv-wasm+locations          | 2.25       | 32  | 445.51   | 446.14   | 448.87   | 464.02   | 465.99   | 442.13   | 466.35   | 1.63x                        | 1.62x    |
| tsv-internal                | 31.29      | 387 | 31.96    | 32.13    | 32.43    | 32.58    | 32.95    | 31.77    | 36.34    | 22.7x                        | 22.7x    |
| tsv-wasm-internal           | 17.78      | 254 | 56.24    | 56.39    | 56.63    | 56.81    | 57.20    | 55.95    | 57.67    | 12.9x                        | 12.9x    |
| rsvelte-parse               | 0.88       | 21  | 1132.99  | 1135.31  | 1145.49  | 1154.27  | 1155.37  | 1125.07  | 1155.44  | 0.64x                        | 0.64x    |
| rsvelte-parse-skip-expr-loc | 1.42       | 24  | 702.44   | 703.59   | 704.90   | 705.50   | 706.62   | 698.46   | 706.93   | 1.03x                        | 1.03x    |

**Files (intersection):** 1913

**Throughput:** svelte/compiler 6.7 MB/s, tsv 18.0 MB/s, tsv-wasm 14.6 MB/s, tsv+locations 12.8 MB/s, tsv-wasm+locations 10.9 MB/s, tsv-internal 152.2 MB/s, tsv-wasm-internal 86.5 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.4x tsv-internal, tsv-wasm 5.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.09       | 23  | 10.83   | 10.96   | 11.09   | 11.10   | 11.13   | 10.60   | 11.14   | baseline              | baseline |
| tsv        | 7.44       | 111 | 0.13    | 0.13    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 80.8x                 | 80.6x    |
| tsv-wasm   | 4.48       | 69  | 0.22    | 0.22    | 0.23    | 0.23    | 0.23    | 0.22    | 0.23    | 48.6x                 | 48.4x    |
| oxfmt      | 0.09       | 23  | 10.99   | 11.02   | 11.06   | 11.07   | 11.07   | 10.85   | 11.07   | 0.99x                 | 0.99x    |
| biome-wasm | 0.53       | 24  | 1.88    | 1.89    | 1.90    | 1.90    | 1.91    | 1.87    | 1.91    | 5.76x                 | 5.75x    |

**Files (intersection):** 1906

**Throughput:** prettier 0.4 MB/s, tsv 35.5 MB/s, tsv-wasm 21.4 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 1913/1913 (100%), tsv 1913/1913 (100%), tsv-wasm 1913/1913 (100%), oxfmt 1913/1913 (100%), biome-wasm 1906/1913 (99%)

**Omitted from every row's timed set:** 7 of 1913 files, 1.8% of the group's bytes (0.4% of its files) — by row: biome-wasm 7 (7 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 1795/1913 (93%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 1913/1913 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.51       | 23 | 1.97    | 1.99    | 2.00    | 2.00    | 2.00    | 1.95    | 2.00    | baseline                      | baseline |
| tsv                | 0.69       | 22 | 1.46    | 1.47    | 1.47    | 1.47    | 1.47    | 1.45    | 1.47    | 1.35x                         | 1.35x    |
| tsv-wasm           | 0.56       | 23 | 1.78    | 1.79    | 1.79    | 1.79    | 1.80    | 1.77    | 1.80    | 1.11x                         | 1.11x    |
| tsv+locations      | 0.54       | 24 | 1.87    | 1.87    | 1.87    | 1.87    | 1.88    | 1.85    | 1.88    | 1.06x                         | 1.05x    |
| tsv-wasm+locations | 0.46       | 22 | 2.16    | 2.20    | 2.20    | 2.20    | 2.21    | 2.15    | 2.21    | 0.91x                         | 0.91x    |
| tsv-internal       | 6.03       | 92 | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.16    | 0.17    | 11.9x                         | 11.9x    |
| tsv-wasm-internal  | 3.34       | 52 | 0.30    | 0.31    | 0.31    | 0.31    | 0.31    | 0.29    | 0.31    | 6.60x                         | 6.59x    |
| oxc-parser         | 0.44       | 24 | 2.27    | 2.27    | 2.28    | 2.29    | 2.29    | 2.25    | 2.30    | 0.87x                         | 0.87x    |
| oxc-parser-wasm    | 0.40       | 24 | 2.51    | 2.52    | 2.53    | 2.53    | 2.54    | 2.48    | 2.54    | 0.79x                         | 0.79x    |
| yuku-parser        | 1.23       | 20 | 0.81    | 0.82    | 0.84    | 0.84    | 0.85    | 0.79    | 0.85    | 2.44x                         | 2.44x    |
| yuku-parser-wasm   | 1.48       | 23 | 0.68    | 0.68    | 0.68    | 0.68    | 0.68    | 0.67    | 0.69    | 2.92x                         | 2.92x    |
| swc                | 0.34       | 24 | 2.96    | 2.97    | 2.97    | 2.98    | 2.98    | 2.93    | 2.98    | 0.67x                         | 0.67x    |

**Files (intersection):** 5697

**Throughput:** acorn-typescript 17.3 MB/s, tsv 23.4 MB/s, tsv-wasm 19.2 MB/s, tsv+locations 18.3 MB/s, tsv-wasm+locations 15.7 MB/s, tsv-internal 205.7 MB/s, tsv-wasm-internal 114.0 MB/s, oxc-parser 15.1 MB/s, oxc-parser-wasm 13.6 MB/s, yuku-parser 42.1 MB/s, yuku-parser-wasm 50.5 MB/s, swc 11.5 MB/s

**Coverage:** acorn-typescript 5697/5700 (99%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), tsv+locations 5700/5700 (100%), tsv-wasm+locations 5700/5700 (100%), tsv-internal 5700/5700 (100%), tsv-wasm-internal 5700/5700 (100%), oxc-parser 5698/5700 (99%), oxc-parser-wasm 5698/5700 (99%), yuku-parser 5698/5700 (99%), yuku-parser-wasm 5698/5700 (99%), swc 5697/5700 (99%)

**Omitted from every row's timed set:** 3 of 5700 files, 0.0% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.8x tsv-internal, tsv-wasm 5.9x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 24 | 25.01   | 25.17   | 25.45   | 25.53   | 25.61   | 24.82   | 25.63   | baseline              | baseline |
| tsv         | 1.42       | 24 | 0.70    | 0.71    | 0.71    | 0.71    | 0.71    | 0.70    | 0.71    | 35.6x                 | 35.5x    |
| tsv-wasm    | 0.85       | 24 | 1.18    | 1.19    | 1.19    | 1.19    | 1.19    | 1.17    | 1.19    | 21.2x                 | 21.2x    |
| oxfmt       | 0.62       | 24 | 1.61    | 1.63    | 1.64    | 1.64    | 1.64    | 1.57    | 1.64    | 15.6x                 | 15.5x    |
| biome-wasm  | 0.12       | 23 | 8.25    | 8.31    | 8.33    | 8.33    | 8.33    | 8.20    | 8.33    | 3.03x                 | 3.03x    |
| dprint-wasm | 0.15       | 22 | 6.79    | 6.90    | 6.91    | 6.91    | 6.91    | 6.74    | 6.92    | 3.68x                 | 3.69x    |

**Files (intersection):** 5697

**Throughput:** prettier 1.4 MB/s, tsv 48.5 MB/s, tsv-wasm 28.9 MB/s, oxfmt 21.2 MB/s, biome-wasm 4.1 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 5700/5700 (100%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), oxfmt 5698/5700 (99%), biome-wasm 5697/5700 (99%), dprint-wasm 5700/5700 (100%)

**Omitted from every row's timed set:** 3 of 5700 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 79.33      | 1173 | 12.58    | 12.80    | 13.02    | 13.18    | 13.62    | 12.02    | 16.00    | baseline                     | baseline |
| tsv                | 45.80      | 664  | 21.76    | 22.02    | 22.53    | 22.67    | 22.93    | 21.34    | 23.62    | 0.58x                        | 0.58x    |
| tsv-wasm           | 37.78      | 516  | 26.42    | 26.69    | 27.19    | 27.35    | 27.54    | 25.98    | 28.38    | 0.48x                        | 0.48x    |
| tsv+locations      | 36.13      | 530  | 27.75    | 28.16    | 28.54    | 29.06    | 32.70    | 26.78    | 36.36    | 0.46x                        | 0.45x    |
| tsv-wasm+locations | 30.82      | 454  | 32.35    | 33.10    | 33.34    | 33.75    | 37.36    | 31.40    | 39.77    | 0.39x                        | 0.39x    |
| tsv-internal       | 318.40     | 4468 | 3.14     | 3.15     | 3.17     | 3.19     | 3.28     | 3.11     | 5.05     | 4.01x                        | 4.01x    |
| tsv-wasm-internal  | 179.64     | 2551 | 5.57     | 5.59     | 5.62     | 5.68     | 5.79     | 5.50     | 6.71     | 2.26x                        | 2.26x    |
| postcss            | 67.02      | 980  | 14.86    | 15.16    | 15.41    | 15.60    | 16.32    | 14.34    | 16.79    | 0.84x                        | 0.85x    |

**Files (intersection):** 73

**Throughput:** svelte/compiler 40.0 MB/s, tsv 23.1 MB/s, tsv-wasm 19.0 MB/s, tsv+locations 18.2 MB/s, tsv-wasm+locations 15.5 MB/s, tsv-internal 160.4 MB/s, tsv-wasm-internal 90.5 MB/s, postcss 33.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.0x tsv-internal, tsv-wasm 4.8x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.43       | 22   | 696.61   | 704.48   | 709.73   | 713.72   | 716.28   | 678.75   | 716.86   | baseline              | baseline |
| tsv        | 174.88     | 2429 | 5.71     | 5.75     | 5.80     | 5.85     | 5.97     | 5.65     | 7.08     | 122.0x                | 121.9x   |
| tsv-wasm   | 97.85      | 1304 | 10.22    | 10.27    | 10.36    | 10.40    | 10.66    | 10.11    | 11.29    | 68.3x                 | 68.1x    |
| oxfmt      | 43.86      | 640  | 22.82    | 23.14    | 23.48    | 23.78    | 24.65    | 20.96    | 26.91    | 30.6x                 | 30.5x    |
| biome-wasm | 8.35       | 114  | 119.27   | 121.28   | 122.57   | 126.32   | 130.63   | 117.54   | 136.28   | 5.82x                 | 5.84x    |
| malva-wasm | 15.28      | 204  | 65.44    | 65.62    | 65.93    | 66.18    | 66.57    | 65.16    | 66.64    | 10.7x                 | 10.6x    |

**Files (intersection):** 71

**Throughput:** prettier 0.7 MB/s, tsv 81.0 MB/s, tsv-wasm 45.4 MB/s, oxfmt 20.3 MB/s, biome-wasm 3.9 MB/s, malva-wasm 7.1 MB/s

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
| tsv (ffi) | 3.6 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.9 MB | 4.5 MB | 3.0x | 2.7x |
| tsv format (ffi) | 3.3 MB | 1.6 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 711.3 KB | 0.4x | 0.4x |
| tsv (napi) | 4.1 MB | 1.9 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 880.8 KB | 0.6x | 0.5x |
| oxfmt (napi) | 8.8 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 681.1 KB | 286.0 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 2.5x | 2.1x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 4.9x | 4.4x |
| swc (napi) | 10.5 MB | 10.2 MB | 2.9x | 6.0x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1906f) | **80.8x** prettier, **81.7x** oxfmt |
| format typescript (5697f) | **35.6x** prettier, **2.29x** oxfmt |
| format css (71f) | **122.0x** prettier, **3.99x** oxfmt |
| parse svelte (1913f) | **2.69x** svelte/compiler, **4.19x** rsvelte-parse |
| parse typescript (5697f) | **1.35x** acorn-typescript, **1.55x** oxc-parser, **0.56x** yuku-parser, **2.03x** swc |
| parse css (73f) | **0.58x** svelte/compiler, **0.68x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1906f) | **48.6x** prettier, **8.43x** biome-wasm |
| format typescript (5697f) | **21.2x** prettier, **7.00x** biome-wasm, **5.77x** dprint-wasm |
| format css (71f) | **68.3x** prettier, **11.7x** biome-wasm, **6.40x** malva-wasm |
| parse svelte (1913f) | **2.17x** svelte/compiler |
| parse typescript (5697f) | **1.11x** acorn-typescript, **1.41x** oxc-parser-wasm, **0.38x** yuku-parser-wasm |
| parse css (73f) | **0.48x** svelte/compiler, **0.56x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.41x, wasm 1.33x | typescript: native 1.28x, wasm 1.22x | css: native 1.27x, wasm 1.23x

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
