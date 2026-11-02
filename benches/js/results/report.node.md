# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-11-02T05:32:15.271Z — tsv 0.8.0 (83aa5582)

**Corpus:** 1898 Svelte (4.8 MB), 5691 TypeScript (34.1 MB), 71 CSS (0.5 MB) — 7660 files, 39.4 MB total

**Corpus snapshot:** [fuzdev/corpora@678f4942f](https://github.com/fuzdev/corpora/tree/678f4942f92ca0fab49a73db37f61546f34b470c) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 57% third-party by bytes (1091 files, 2.8 MB third-party · 807 files, 2.1 MB the author's own); TypeScript 57% third-party by bytes (3895 files, 19.3 MB third-party · 1796 files, 14.8 MB the author's own); CSS 48% third-party by bytes (32 files, 0.2 MB third-party · 39 files, 0.3 MB the author's own)

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (532), ../corpora/collections/layercake/src (246), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (149), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/core/src (1088), ../corpora/collections/storybook/code/lib/cli-storybook/src (111), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (75), ../corpora/collections/storybook/code/lib/csf-plugin/src (4), ../corpora/collections/storybook/code/lib/eslint-plugin/src (47), ../corpora/collections/storybook/code/lib/react-dom-shim/src (1), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (24)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.7% apart at the median, 3.9% at the 95th percentile and 10.5% at most (141 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.29       | 24  | 774.04   | 783.26   | 790.28   | 792.49   | 794.26   | 759.58   | 794.69   | baseline                     | baseline |
| tsv                         | 3.35       | 51  | 297.43   | 300.73   | 301.94   | 302.03   | 302.20   | 295.25   | 302.21   | 2.60x                        | 2.60x    |
| tsv-wasm                    | 2.98       | 45  | 335.54   | 337.54   | 339.15   | 339.81   | 340.27   | 333.27   | 340.57   | 2.31x                        | 2.31x    |
| tsv+locations               | 2.53       | 35  | 395.15   | 397.85   | 399.40   | 402.67   | 412.98   | 390.72   | 414.39   | 1.96x                        | 1.96x    |
| tsv-wasm+locations          | 2.31       | 33  | 432.25   | 436.48   | 438.03   | 438.85   | 442.48   | 428.63   | 444.38   | 1.79x                        | 1.79x    |
| tsv-internal                | 29.30      | 400 | 34.12    | 34.22    | 34.44    | 34.64    | 35.01    | 33.90    | 36.01    | 22.7x                        | 22.7x    |
| tsv-wasm-internal           | 19.15      | 257 | 52.18    | 52.78    | 53.00    | 53.37    | 54.05    | 51.46    | 54.31    | 14.8x                        | 14.8x    |
| rsvelte-parse               | 0.84       | 23  | 1197.16  | 1202.56  | 1205.39  | 1205.87  | 1206.06  | 1188.72  | 1206.12  | 0.65x                        | 0.65x    |
| rsvelte-parse-skip-expr-loc | 1.36       | 23  | 738.51   | 739.29   | 741.11   | 741.75   | 742.14   | 734.27   | 742.24   | 1.05x                        | 1.05x    |

**Files (intersection):** 1898

**Throughput:** svelte/compiler 6.2 MB/s, tsv 16.2 MB/s, tsv-wasm 14.4 MB/s, tsv+locations 12.2 MB/s, tsv-wasm+locations 11.2 MB/s, tsv-internal 141.7 MB/s, tsv-wasm-internal 92.6 MB/s, rsvelte-parse 4.0 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.7x tsv-internal, tsv-wasm 6.4x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.09       | 24 | 11.33   | 11.39   | 11.43   | 11.43   | 11.43   | 11.16   | 11.43   | baseline              | baseline |
| tsv        | 7.19       | 98 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 81.5x                 | 81.4x    |
| tsv-wasm   | 4.98       | 75 | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 56.5x                 | 56.5x    |
| oxfmt      | 0.09       | 24 | 11.59   | 11.64   | 11.68   | 11.69   | 11.72   | 11.45   | 11.72   | 0.98x                 | 0.98x    |
| biome-wasm | 0.45       | 21 | 2.21    | 2.22    | 2.35    | 2.40    | 2.41    | 2.18    | 2.41    | 5.13x                 | 5.13x    |

**Files (intersection):** 1891

**Throughput:** prettier 0.4 MB/s, tsv 34.1 MB/s, tsv-wasm 23.7 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 1898/1898 (100%), tsv 1898/1898 (100%), tsv-wasm 1898/1898 (100%), oxfmt 1898/1898 (100%), biome-wasm 1891/1898 (99%)

**Omitted from every row's timed set:** 7 of 1898 files, 1.8% of the group's bytes (0.4% of its files) — by row: biome-wasm 7 (7 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 1828/1898 (96%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 1898/1898 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.39       | 22 | 2.58    | 2.60    | 2.61    | 2.61    | 2.62    | 2.56    | 2.62    | baseline                      | baseline |
| tsv                | 0.58       | 24 | 1.72    | 1.75    | 1.75    | 1.75    | 1.75    | 1.71    | 1.75    | 1.50x                         | 1.50x    |
| tsv-wasm           | 0.54       | 21 | 1.86    | 1.87    | 1.87    | 1.87    | 1.88    | 1.85    | 1.88    | 1.39x                         | 1.38x    |
| tsv+locations      | 0.48       | 24 | 2.08    | 2.09    | 2.10    | 2.10    | 2.10    | 2.07    | 2.10    | 1.24x                         | 1.24x    |
| tsv-wasm+locations | 0.45       | 21 | 2.24    | 2.25    | 2.25    | 2.26    | 2.26    | 2.23    | 2.26    | 1.15x                         | 1.15x    |
| tsv-internal       | 5.20       | 79 | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 13.4x                         | 13.4x    |
| tsv-wasm-internal  | 3.57       | 55 | 0.28    | 0.28    | 0.28    | 0.28    | 0.28    | 0.27    | 0.29    | 9.22x                         | 9.12x    |
| oxc-parser         | 0.39       | 22 | 2.56    | 2.57    | 2.57    | 2.57    | 2.59    | 2.53    | 2.59    | 1.01x                         | 1.01x    |
| oxc-parser-wasm    | 0.37       | 24 | 2.68    | 2.69    | 2.70    | 2.70    | 2.70    | 2.65    | 2.70    | 0.96x                         | 0.96x    |
| oxc-parser-raw     | 1.53       | 20 | 0.65    | 0.68    | 0.68    | 0.70    | 0.70    | 0.59    | 0.70    | 3.96x                         | 3.94x    |
| yuku-parser        | 1.48       | 24 | 0.67    | 0.69    | 0.70    | 0.70    | 0.70    | 0.65    | 0.70    | 3.83x                         | 3.83x    |
| yuku-parser-wasm   | 1.93       | 30 | 0.52    | 0.52    | 0.53    | 0.53    | 0.53    | 0.51    | 0.53    | 4.99x                         | 4.98x    |
| swc                | 0.29       | 24 | 3.28    | 3.62    | 3.63    | 3.63    | 3.63    | 3.28    | 3.63    | 0.76x                         | 0.78x    |

**Files (intersection):** 5688

**Throughput:** acorn-typescript 13.2 MB/s, tsv 19.7 MB/s, tsv-wasm 18.3 MB/s, tsv+locations 16.4 MB/s, tsv-wasm+locations 15.2 MB/s, tsv-internal 177.1 MB/s, tsv-wasm-internal 121.6 MB/s, oxc-parser 13.3 MB/s, oxc-parser-wasm 12.7 MB/s, oxc-parser-raw 52.3 MB/s, yuku-parser 50.5 MB/s, yuku-parser-wasm 65.9 MB/s, swc 10.0 MB/s

**Coverage:** acorn-typescript 5688/5691 (99%), tsv 5691/5691 (100%), tsv-wasm 5691/5691 (100%), tsv+locations 5691/5691 (100%), tsv-wasm+locations 5691/5691 (100%), tsv-internal 5691/5691 (100%), tsv-wasm-internal 5691/5691 (100%), oxc-parser 5689/5691 (99%), oxc-parser-wasm 5689/5691 (99%), oxc-parser-raw 5689/5691 (99%), yuku-parser 5689/5691 (99%), yuku-parser-wasm 5689/5691 (99%), swc 5688/5691 (99%)

**Omitted from every row's timed set:** 3 of 5691 files, 0.0% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); oxc-parser-raw 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.0x tsv-internal, tsv-wasm 6.6x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 24 | 27.44   | 27.51   | 27.55   | 27.65   | 27.70   | 26.82   | 27.71   | baseline              | baseline |
| tsv         | 1.30       | 24 | 0.77    | 0.77    | 0.77    | 0.77    | 0.77    | 0.77    | 0.77    | 35.6x                 | 35.7x    |
| tsv-wasm    | 0.92       | 24 | 1.09    | 1.10    | 1.10    | 1.10    | 1.10    | 1.08    | 1.10    | 25.1x                 | 25.2x    |
| oxfmt       | 0.72       | 24 | 1.39    | 1.39    | 1.45    | 1.45    | 1.46    | 1.36    | 1.46    | 19.6x                 | 19.8x    |
| biome-wasm  | 0.11       | 21 | 8.82    | 8.85    | 9.09    | 9.19    | 9.21    | 8.72    | 9.21    | 3.10x                 | 3.11x    |
| dprint-wasm | 0.17       | 24 | 6.04    | 6.08    | 6.09    | 6.09    | 6.09    | 5.99    | 6.09    | 4.52x                 | 4.55x    |

**Files (intersection):** 5688

**Throughput:** prettier 1.2 MB/s, tsv 44.4 MB/s, tsv-wasm 31.3 MB/s, oxfmt 24.4 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 5691/5691 (100%), tsv 5691/5691 (100%), tsv-wasm 5691/5691 (100%), oxfmt 5689/5691 (99%), biome-wasm 5688/5691 (99%), dprint-wasm 5691/5691 (100%)

**Omitted from every row's timed set:** 3 of 5691 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 76.99      | 1057 | 12.91    | 13.21    | 13.56    | 13.72    | 13.89    | 12.63    | 14.27    | baseline                     | baseline |
| tsv                | 39.11      | 462  | 25.38    | 26.10    | 26.20    | 26.30    | 26.95    | 25.00    | 27.20    | 0.51x                        | 0.51x    |
| tsv-wasm           | 37.90      | 464  | 26.22    | 26.90    | 28.46    | 29.74    | 31.06    | 25.92    | 32.36    | 0.49x                        | 0.49x    |
| tsv+locations      | 33.74      | 369  | 29.72    | 31.10    | 32.51    | 32.65    | 33.30    | 28.86    | 33.77    | 0.44x                        | 0.43x    |
| tsv-wasm+locations | 32.52      | 354  | 30.79    | 31.50    | 33.06    | 33.43    | 35.20    | 30.46    | 35.39    | 0.42x                        | 0.42x    |
| tsv-internal       | 292.00     | 4081 | 3.43     | 3.44     | 3.46     | 3.48     | 3.56     | 3.34     | 3.88     | 3.79x                        | 3.76x    |
| tsv-wasm-internal  | 202.17     | 2601 | 4.95     | 4.97     | 5.02     | 5.06     | 5.14     | 4.89     | 5.82     | 2.63x                        | 2.61x    |
| postcss            | 65.17      | 923  | 15.33    | 15.55    | 15.88    | 16.04    | 16.33    | 14.99    | 16.66    | 0.85x                        | 0.84x    |

**Files (intersection):** 71

**Throughput:** svelte/compiler 38.6 MB/s, tsv 19.6 MB/s, tsv-wasm 19.0 MB/s, tsv+locations 16.9 MB/s, tsv-wasm+locations 16.3 MB/s, tsv-internal 146.3 MB/s, tsv-wasm-internal 101.3 MB/s, postcss 32.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.5x tsv-internal, tsv-wasm 5.3x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.41       | 23   | 715.61   | 725.46   | 732.16   | 732.80   | 735.09   | 685.10   | 735.76   | baseline              | baseline |
| tsv        | 158.09     | 2119 | 6.33     | 6.35     | 6.42     | 6.80     | 6.89     | 6.26     | 7.26     | 112.3x                | 113.1x   |
| tsv-wasm   | 113.14     | 1378 | 8.85     | 8.89     | 9.11     | 9.18     | 9.46     | 8.76     | 12.84    | 80.4x                 | 80.9x    |
| oxfmt      | 44.72      | 662  | 22.33    | 22.70    | 23.07    | 23.40    | 24.18    | 21.08    | 25.04    | 31.8x                 | 32.0x    |
| biome-wasm | 10.35      | 111  | 96.35    | 101.08   | 103.09   | 103.68   | 104.73   | 94.80    | 105.15   | 7.35x                 | 7.43x    |
| malva-wasm | 16.64      | 222  | 60.07    | 60.27    | 60.54    | 60.70    | 61.05    | 59.79    | 64.96    | 11.8x                 | 11.9x    |

**Files (intersection):** 69

**Throughput:** prettier 0.6 MB/s, tsv 72.9 MB/s, tsv-wasm 52.1 MB/s, oxfmt 20.6 MB/s, biome-wasm 4.8 MB/s, malva-wasm 7.7 MB/s

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
| format svelte (1891f) | **81.5x** prettier, **83.3x** oxfmt |
| format typescript (5688f) | **35.6x** prettier, **1.82x** oxfmt |
| format css (69f) | **112.3x** prettier, **3.54x** oxfmt |
| parse svelte (1898f) | **2.60x** svelte/compiler, **4.02x** rsvelte-parse |
| parse typescript (5688f) | **1.50x** acorn-typescript, **1.48x** oxc-parser, **0.38x** oxc-parser-raw, **0.39x** yuku-parser, **1.97x** swc |
| parse css (71f) | **0.51x** svelte/compiler, **0.60x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1891f) | **56.5x** prettier, **11.0x** biome-wasm |
| format typescript (5688f) | **25.1x** prettier, **8.08x** biome-wasm, **5.55x** dprint-wasm |
| format css (69f) | **80.4x** prettier, **10.9x** biome-wasm, **6.80x** malva-wasm |
| parse svelte (1898f) | **2.31x** svelte/compiler |
| parse typescript (5688f) | **1.39x** acorn-typescript, **1.44x** oxc-parser-wasm, **0.28x** yuku-parser-wasm |
| parse css (71f) | **0.49x** svelte/compiler, **0.58x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. oxc-parser-raw is the same native parse as oxc-parser and hands JS the identical tree (byte parity graded per file), but crosses the boundary as a shared buffer that a generated JS deserializer builds the objects from instead of a JSON string — oxc’s undocumented `experimentalRawTransfer`. Deserialization is eager and complete before `parseSync` returns, so the row materializes the full tree like tsv’s default rows, by a transfer mechanism tsv does not ship. Native only, and absent under Bun, where oxc does not support raw transfer. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.32x, wasm 1.29x | typescript: native 1.21x, wasm 1.20x | css: native 1.16x, wasm 1.17x

## Unstable Rows

2 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse/typescript/swc | 4.9% | 4.9% | -0.2% | 10.5% | 24/24 |
| parse/typescript/oxc-parser-raw | 4.5% | 5.0% | +4.8% | 8.5% | 20/24 |

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
- parse/typescript: oxc-parser-raw: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2
- format/css: biome-wasm: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
