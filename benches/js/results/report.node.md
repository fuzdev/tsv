# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-18T05:27:37.267Z — tsv 0.7.0 (7103b4aa)

**Corpus:** 1913 Svelte (4.9 MB), 5700 TypeScript (34.1 MB), 73 CSS (0.5 MB) — 7686 files, 39.5 MB total

**Corpus snapshot:** [fuzdev/corpora@976ba903d](https://github.com/fuzdev/corpora/tree/976ba903d97b2ceb236df9a511dcdea1591843e4) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 57% third-party by bytes (1091 files, 2.8 MB third-party · 822 files, 2.1 MB the author's own); TypeScript 56% third-party by bytes (3895 files, 19.3 MB third-party · 1805 files, 14.9 MB the author's own); CSS 47% third-party by bytes (32 files, 0.2 MB third-party · 41 files, 0.3 MB the author's own)

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (532), ../corpora/collections/layercake/src (246), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (149), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/core/src (1088), ../corpora/collections/storybook/code/lib/cli-storybook/src (111), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (75), ../corpora/collections/storybook/code/lib/csf-plugin/src (4), ../corpora/collections/storybook/code/lib/eslint-plugin/src (47), ../corpora/collections/storybook/code/lib/react-dom-shim/src (1), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (25)

**Versions:** svelte@5.57.2, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.9, prettier-plugin-svelte@4.1.1, oxc-parser@0.153.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.72.0, yuku-parser@0.17.0, @biomejs/wasm-bundler@2.5.15, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.5, postcss@8.5.29, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.13

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 2.5% at the 95th percentile and 12.7% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.26       | 22  | 795.10   | 799.74   | 805.05   | 806.63   | 811.75   | 769.31   | 813.25   | baseline                     | baseline |
| tsv                         | 3.33       | 51  | 300.09   | 300.67   | 301.34   | 301.43   | 302.05   | 298.72   | 302.59   | 2.64x                        | 2.65x    |
| tsv-wasm                    | 2.99       | 45  | 334.59   | 335.52   | 336.34   | 336.75   | 337.39   | 332.97   | 337.51   | 2.37x                        | 2.38x    |
| tsv+locations               | 2.53       | 39  | 395.55   | 396.32   | 396.94   | 396.99   | 398.20   | 390.26   | 398.94   | 2.01x                        | 2.01x    |
| tsv-wasm+locations          | 2.33       | 33  | 430.02   | 434.05   | 434.91   | 443.21   | 449.37   | 423.05   | 450.68   | 1.84x                        | 1.85x    |
| tsv-internal                | 30.72      | 423 | 32.56    | 32.65    | 32.80    | 33.02    | 33.29    | 32.29    | 33.52    | 24.3x                        | 24.4x    |
| tsv-wasm-internal           | 19.87      | 277 | 50.36    | 50.47    | 50.69    | 50.91    | 51.20    | 49.88    | 51.61    | 15.7x                        | 15.8x    |
| rsvelte-parse               | 0.83       | 22  | 1200.91  | 1204.48  | 1206.94  | 1208.02  | 1217.56  | 1197.61  | 1220.36  | 0.66x                        | 0.66x    |
| rsvelte-parse-skip-expr-loc | 1.34       | 24  | 743.96   | 746.48   | 747.57   | 748.45   | 749.15   | 740.68   | 749.32   | 1.06x                        | 1.07x    |

**Files (intersection):** 1913

**Throughput:** svelte/compiler 6.1 MB/s, tsv 16.2 MB/s, tsv-wasm 14.5 MB/s, tsv+locations 12.3 MB/s, tsv-wasm+locations 11.3 MB/s, tsv-internal 149.4 MB/s, tsv-wasm-internal 96.7 MB/s, rsvelte-parse 4.0 MB/s, rsvelte-parse-skip-expr-loc 6.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.2x tsv-internal, tsv-wasm 6.7x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.09       | 24  | 11.45   | 11.47   | 11.52   | 11.56   | 11.57   | 11.28   | 11.57   | baseline              | baseline |
| tsv        | 7.42       | 111 | 0.13    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 84.8x                 | 85.0x    |
| tsv-wasm   | 5.10       | 77  | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.19    | 0.20    | 58.3x                 | 58.4x    |
| oxfmt      | 0.09       | 24  | 11.59   | 11.64   | 11.68   | 11.69   | 11.75   | 11.47   | 11.77   | 0.99x                 | 0.99x    |
| biome-wasm | 0.49       | 21  | 2.04    | 2.05    | 2.17    | 2.25    | 2.26    | 2.02    | 2.27    | 5.60x                 | 5.60x    |

**Files (intersection):** 1906

**Throughput:** prettier 0.4 MB/s, tsv 35.4 MB/s, tsv-wasm 24.4 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.3 MB/s

**Coverage:** prettier 1913/1913 (100%), tsv 1913/1913 (100%), tsv-wasm 1913/1913 (100%), oxfmt 1913/1913 (100%), biome-wasm 1906/1913 (99%)

**Omitted from every row's timed set:** 7 of 1913 files, 1.8% of the group's bytes (0.4% of its files) — by row: biome-wasm 7 (7 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 1795/1913 (93%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 1913/1913 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.39       | 22 | 2.60    | 2.61    | 2.61    | 2.61    | 2.61    | 2.53    | 2.61    | baseline                      | baseline |
| tsv                | 0.59       | 24 | 1.70    | 1.71    | 1.71    | 1.71    | 1.71    | 1.69    | 1.71    | 1.52x                         | 1.53x    |
| tsv-wasm           | 0.54       | 23 | 1.83    | 1.84    | 1.84    | 1.85    | 1.85    | 1.83    | 1.85    | 1.40x                         | 1.42x    |
| tsv+locations      | 0.48       | 23 | 2.07    | 2.08    | 2.09    | 2.09    | 2.09    | 2.06    | 2.09    | 1.24x                         | 1.25x    |
| tsv-wasm+locations | 0.45       | 19 | 2.22    | 2.23    | 2.23    | 2.24    | 2.24    | 2.20    | 2.24    | 1.16x                         | 1.17x    |
| tsv-internal       | 5.57       | 83 | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 14.4x                         | 14.5x    |
| tsv-wasm-internal  | 3.81       | 58 | 0.26    | 0.26    | 0.26    | 0.26    | 0.27    | 0.26    | 0.27    | 9.83x                         | 9.91x    |
| oxc-parser         | 0.40       | 24 | 2.52    | 2.53    | 2.54    | 2.55    | 2.55    | 2.50    | 2.56    | 1.02x                         | 1.03x    |
| oxc-parser-wasm    | 0.38       | 21 | 2.65    | 2.66    | 2.66    | 2.67    | 2.67    | 2.63    | 2.67    | 0.97x                         | 0.98x    |
| yuku-parser        | 1.43       | 23 | 0.70    | 0.70    | 0.70    | 0.71    | 0.71    | 0.69    | 0.71    | 3.69x                         | 3.72x    |
| yuku-parser-wasm   | 1.85       | 27 | 0.54    | 0.54    | 0.55    | 0.55    | 0.55    | 0.54    | 0.55    | 4.77x                         | 4.79x    |
| swc                | 0.30       | 23 | 3.23    | 3.63    | 3.63    | 3.64    | 3.64    | 3.22    | 3.64    | 0.77x                         | 0.81x    |

**Files (intersection):** 5697

**Throughput:** acorn-typescript 13.2 MB/s, tsv 20.0 MB/s, tsv-wasm 18.6 MB/s, tsv+locations 16.4 MB/s, tsv-wasm+locations 15.4 MB/s, tsv-internal 190.0 MB/s, tsv-wasm-internal 130.1 MB/s, oxc-parser 13.5 MB/s, oxc-parser-wasm 12.9 MB/s, yuku-parser 48.9 MB/s, yuku-parser-wasm 63.0 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 5697/5700 (99%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), tsv+locations 5700/5700 (100%), tsv-wasm+locations 5700/5700 (100%), tsv-internal 5700/5700 (100%), tsv-wasm-internal 5700/5700 (100%), oxc-parser 5698/5700 (99%), oxc-parser-wasm 5698/5700 (99%), yuku-parser 5698/5700 (99%), yuku-parser-wasm 5698/5700 (99%), swc 5697/5700 (99%)

**Omitted from every row's timed set:** 3 of 5700 files, 0.0% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.5x tsv-internal, tsv-wasm 7.0x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 22 | 27.38   | 27.63   | 28.13   | 28.17   | 28.24   | 27.15   | 28.26   | baseline              | baseline |
| tsv         | 1.35       | 24 | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 37.4x                 | 37.1x    |
| tsv-wasm    | 0.96       | 24 | 1.04    | 1.05    | 1.05    | 1.05    | 1.05    | 1.04    | 1.05    | 26.4x                 | 26.3x    |
| oxfmt       | 0.62       | 23 | 1.61    | 1.62    | 1.63    | 1.64    | 1.67    | 1.59    | 1.68    | 17.1x                 | 17.0x    |
| biome-wasm  | 0.12       | 21 | 8.04    | 8.10    | 8.27    | 8.36    | 8.46    | 7.99    | 8.49    | 3.43x                 | 3.41x    |
| dprint-wasm | 0.17       | 24 | 5.99    | 6.04    | 6.06    | 6.06    | 6.06    | 5.96    | 6.06    | 4.59x                 | 4.57x    |

**Files (intersection):** 5697

**Throughput:** prettier 1.2 MB/s, tsv 46.2 MB/s, tsv-wasm 32.7 MB/s, oxfmt 21.1 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 5700/5700 (100%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), oxfmt 5698/5700 (99%), biome-wasm 5697/5700 (99%), dprint-wasm 5700/5700 (100%)

**Omitted from every row's timed set:** 3 of 5700 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 76.88      | 1061 | 12.95    | 13.37    | 13.70    | 13.82    | 14.25    | 12.48    | 17.28    | baseline                     | baseline |
| tsv                | 39.79      | 473  | 25.21    | 25.49    | 28.16    | 28.53    | 29.45    | 24.36    | 33.63    | 0.52x                        | 0.51x    |
| tsv-wasm           | 37.45      | 418  | 26.80    | 27.34    | 29.21    | 30.95    | 31.80    | 26.18    | 35.48    | 0.49x                        | 0.48x    |
| tsv+locations      | 33.69      | 371  | 29.78    | 31.24    | 32.54    | 32.72    | 33.13    | 28.89    | 34.24    | 0.44x                        | 0.43x    |
| tsv-wasm+locations | 32.33      | 358  | 30.97    | 31.64    | 33.20    | 33.47    | 35.55    | 30.64    | 36.03    | 0.42x                        | 0.42x    |
| tsv-internal       | 298.26     | 4155 | 3.35     | 3.36     | 3.37     | 3.38     | 3.48     | 3.32     | 7.37     | 3.88x                        | 3.86x    |
| tsv-wasm-internal  | 200.96     | 2818 | 4.98     | 4.99     | 5.02     | 5.05     | 5.18     | 4.93     | 9.01     | 2.61x                        | 2.60x    |
| postcss            | 65.72      | 865  | 15.25    | 15.47    | 15.94    | 16.25    | 16.93    | 14.82    | 17.37    | 0.85x                        | 0.85x    |

**Files (intersection):** 73

**Throughput:** svelte/compiler 38.7 MB/s, tsv 20.0 MB/s, tsv-wasm 18.9 MB/s, tsv+locations 17.0 MB/s, tsv-wasm+locations 16.3 MB/s, tsv-internal 150.3 MB/s, tsv-wasm-internal 101.2 MB/s, postcss 33.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.5x tsv-internal, tsv-wasm 5.4x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.38       | 22   | 725.97   | 728.23   | 741.19   | 745.34   | 757.85   | 711.76   | 761.55   | baseline              | baseline |
| tsv        | 157.39     | 2097 | 6.35     | 6.38     | 6.44     | 6.83     | 6.92     | 6.28     | 7.05     | 113.9x                | 114.2x   |
| tsv-wasm   | 110.85     | 1424 | 9.06     | 9.11     | 9.24     | 9.40     | 9.63     | 8.84     | 10.06    | 80.2x                 | 80.1x    |
| oxfmt      | 43.01      | 640  | 23.25    | 23.57    | 24.04    | 24.34    | 25.07    | 21.58    | 25.94    | 31.1x                 | 31.2x    |
| biome-wasm | 9.03       | 114  | 109.67   | 115.13   | 116.67   | 117.07   | 117.43   | 108.16   | 118.19   | 6.54x                 | 6.62x    |
| malva-wasm | 16.63      | 204  | 60.21    | 60.42    | 60.76    | 60.89    | 61.40    | 59.80    | 61.71    | 12.0x                 | 12.1x    |

**Files (intersection):** 71

**Throughput:** prettier 0.6 MB/s, tsv 72.9 MB/s, tsv-wasm 51.4 MB/s, oxfmt 19.9 MB/s, biome-wasm 4.2 MB/s, malva-wasm 7.7 MB/s

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
| format svelte (1906f) | **84.8x** prettier, **86.0x** oxfmt |
| format typescript (5697f) | **37.4x** prettier, **2.19x** oxfmt |
| format css (71f) | **113.9x** prettier, **3.66x** oxfmt |
| parse svelte (1913f) | **2.64x** svelte/compiler, **4.01x** rsvelte-parse |
| parse typescript (5697f) | **1.52x** acorn-typescript, **1.48x** oxc-parser, **0.41x** yuku-parser, **1.97x** swc |
| parse css (73f) | **0.52x** svelte/compiler, **0.61x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1906f) | **58.3x** prettier, **10.4x** biome-wasm |
| format typescript (5697f) | **26.4x** prettier, **7.71x** biome-wasm, **5.75x** dprint-wasm |
| format css (71f) | **80.2x** prettier, **12.3x** biome-wasm, **6.66x** malva-wasm |
| parse svelte (1913f) | **2.37x** svelte/compiler |
| parse typescript (5697f) | **1.40x** acorn-typescript, **1.44x** oxc-parser-wasm, **0.29x** yuku-parser-wasm |
| parse css (73f) | **0.49x** svelte/compiler, **0.57x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.32x, wasm 1.28x | typescript: native 1.22x, wasm 1.21x | css: native 1.18x, wasm 1.16x

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse/typescript/swc | 5.9% | 5.9% | +0.1% | 12.7% | 23/24 |

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
