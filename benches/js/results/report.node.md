# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-11-07T05:39:05.072Z — tsv 0.8.1 (872e4d3a)

**Corpus:** 2331 Svelte (4.4 MB), 5700 TypeScript (27.0 MB), 192 CSS (0.8 MB) — 8223 files, 32.2 MB total

**Corpus snapshot:** [fuzdev/corpora@dce40bd21](https://github.com/fuzdev/corpora/tree/dce40bd21d5e14fb13246db0ee730120f137cbb8) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 100% third-party by bytes (2331 files, 4.4 MB third-party · 0 files, 0.0 MB the author's own); TypeScript 100% third-party by bytes (5700 files, 27.0 MB third-party · 0 files, 0.0 MB the author's own); CSS 100% third-party by bytes (192 files, 0.8 MB third-party · 0 files, 0.0 MB the author's own)

**Native build:** the native tsv rows (N-API, under Node/Bun) measure the N-API addon's profile-guided build, trained on the corpus snapshot's `origin: own` collections — none of which this run’s corpus holds (zzz, fuz_app, fuz_blog, fuz_code, fuz_css, docs.fuz.dev, fuz_repos, fuz_template, fuz_ui, fuz_util, mdz, gro, svelte-docinfo, tsv.fuz.dev, ryanatkn.com, earbetter, cosmicplayground). The N-API addon's is the shipped addon's: the addon itself — parse on every file and format --jobs 1 over the tree through the @fuzdev/tsv loader’s cli.js, the type strip and import records in process — the training the published addon gets. WASM rows are not profile-guided.

**Sources:** ../corpora/collections/kit/packages/kit/src (316), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (153), ../corpora/collections/svelte.dev/packages/repl/src (55), ../corpora/collections/svelte.dev/packages/site-kit/src (75), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (656), ../corpora/collections/layercake/src (273), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (168), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/bits-ui/packages/bits-ui/src/lib (507), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/ui (461), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/hooks (2), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/lib (3), ../corpora/collections/shadcn-svelte/packages/cli/src (49), ../corpora/collections/shadcn-svelte/packages/primitives/src (49), ../corpora/collections/skeleton/packages/skeleton/src (27), ../corpora/collections/skeleton/packages/skeleton-common/src (27), ../corpora/collections/skeleton/packages/skeleton-svelte/src (449), ../corpora/collections/skeleton/packages/cli/src (37), ../corpora/collections/xyflow/packages/svelte/src/lib (129), ../corpora/collections/xyflow/packages/svelte/src/styles (2), ../corpora/collections/xyflow/packages/system/src (49), ../corpora/collections/threlte/packages/core/src/lib (62), ../corpora/collections/threlte/packages/extras/src/lib (278), ../corpora/collections/threlte/packages/flex/src/lib (20), ../corpora/collections/threlte/packages/rapier/src/lib (42), ../corpora/collections/threlte/packages/studio/src/lib (105), ../corpora/collections/threlte/packages/theatre/src/lib (40), ../corpora/collections/threlte/packages/xr/src/lib (55), ../corpora/collections/runed/packages/runed/src/lib (105), ../corpora/collections/daisyui/packages/daisyui/src (72), ../corpora/collections/open-props/src/extra (16), ../corpora/collections/open-props/src/props.highlights.css (1), ../corpora/collections/open-props/src/props.layouts.css (1), ../corpora/collections/open-props/src/props.shadows.dark.css (1), ../corpora/collections/open-props/src/props.shadows.light.css (1), ../corpora/collections/open-props/src/props.supports.css (1), ../corpora/collections/open-props/docsite/index.css (1), ../corpora/collections/open-props/docsite/color-copy.css (1), ../corpora/collections/open-props/docsite/palette-playground.css (1), ../corpora/collections/open-props/docsite/syntax-highlight.css (1), ../corpora/collections/open-props/docsite/js (11), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/addons/docs/src/csf-plugin (6), ../corpora/collections/storybook/code/core/src (1361), ../corpora/collections/storybook/code/lib/cli-storybook/src (151), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (81), ../corpora/collections/storybook/code/lib/eslint-plugin/src (49), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (9)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.6% apart at the median, 2.6% at the 95th percentile and 6.8% at most (141 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.26       | 24  | 789.56   | 806.37   | 811.87   | 815.67   | 821.17   | 777.72   | 822.67   | baseline                     | baseline |
| tsv                         | 3.65       | 57  | 273.00   | 276.84   | 278.01   | 278.19   | 279.12   | 269.29   | 279.83   | 2.90x                        | 2.89x    |
| tsv-wasm                    | 3.15       | 48  | 317.80   | 318.62   | 318.94   | 319.28   | 320.60   | 315.58   | 321.27   | 2.50x                        | 2.48x    |
| tsv+locations               | 2.76       | 37  | 362.59   | 364.19   | 364.95   | 373.66   | 376.73   | 359.67   | 376.84   | 2.19x                        | 2.18x    |
| tsv-wasm+locations          | 2.44       | 36  | 409.52   | 412.08   | 415.07   | 424.95   | 428.35   | 406.27   | 429.22   | 1.94x                        | 1.93x    |
| tsv-internal                | 36.83      | 499 | 27.16    | 27.24    | 27.44    | 27.63    | 28.06    | 26.93    | 30.43    | 29.2x                        | 29.1x    |
| tsv-wasm-internal           | 19.81      | 269 | 50.60    | 50.72    | 50.97    | 51.27    | 51.62    | 49.94    | 51.87    | 15.7x                        | 15.6x    |
| rsvelte-parse               | 0.90       | 22  | 1109.21  | 1111.64  | 1112.76  | 1113.27  | 1114.73  | 1105.30  | 1115.15  | 0.72x                        | 0.71x    |
| rsvelte-parse-skip-expr-loc | 1.54       | 24  | 645.29   | 656.30   | 657.96   | 658.32   | 658.39   | 642.21   | 658.40   | 1.22x                        | 1.22x    |

**Files (intersection):** 2331

**Throughput:** svelte/compiler 5.5 MB/s, tsv 16.0 MB/s, tsv-wasm 13.8 MB/s, tsv+locations 12.1 MB/s, tsv-wasm+locations 10.7 MB/s, tsv-internal 161.4 MB/s, tsv-wasm-internal 86.8 MB/s, rsvelte-parse 3.9 MB/s, rsvelte-parse-skip-expr-loc 6.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 10.1x tsv-internal, tsv-wasm 6.3x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.09       | 22  | 11.44   | 11.51   | 11.55   | 11.57   | 11.59   | 11.34   | 11.59   | baseline              | baseline |
| tsv        | 10.78      | 144 | 0.09    | 0.09    | 0.09    | 0.09    | 0.10    | 0.09    | 0.10    | 123.3x                | 123.4x   |
| tsv-wasm   | 5.16       | 77  | 0.19    | 0.20    | 0.20    | 0.20    | 0.20    | 0.19    | 0.20    | 59.0x                 | 59.0x    |
| oxfmt      | 0.09       | 24  | 11.22   | 11.25   | 11.28   | 11.29   | 11.33   | 11.14   | 11.34   | 1.02x                 | 1.02x    |
| biome-wasm | 0.43       | 20  | 2.34    | 2.35    | 2.52    | 2.61    | 2.63    | 2.32    | 2.64    | 4.89x                 | 4.88x    |

**Files (intersection):** 2329

**Throughput:** prettier 0.4 MB/s, tsv 47.2 MB/s, tsv-wasm 22.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 1.9 MB/s

**Coverage:** prettier 2331/2331 (100%), tsv 2331/2331 (100%), tsv-wasm 2331/2331 (100%), oxfmt 2331/2331 (100%), biome-wasm 2329/2331 (99%)

**Omitted from every row's timed set:** 2 of 2331 files, 0.1% of the group's bytes (0.1% of its files) — by row: biome-wasm 2 (2 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 2267/2331 (97%) — it rejects valid Svelte in the real corpus (a quoted `style` attribute whose value is no declaration list, some inline arrow-function event handlers), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 2331/2331 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.45       | 21  | 2.23    | 2.24    | 2.24    | 2.24    | 2.25    | 2.22    | 2.25    | baseline                      | baseline |
| tsv                | 0.72       | 24  | 1.38    | 1.39    | 1.39    | 1.39    | 1.40    | 1.38    | 1.40    | 1.61x                         | 1.61x    |
| tsv-wasm           | 0.65       | 22  | 1.54    | 1.57    | 1.57    | 1.57    | 1.59    | 1.53    | 1.60    | 1.44x                         | 1.45x    |
| tsv+locations      | 0.59       | 24  | 1.70    | 1.70    | 1.70    | 1.70    | 1.71    | 1.69    | 1.71    | 1.31x                         | 1.31x    |
| tsv-wasm+locations | 0.53       | 24  | 1.88    | 1.89    | 1.91    | 1.91    | 1.92    | 1.85    | 1.92    | 1.19x                         | 1.19x    |
| tsv-internal       | 7.14       | 104 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 15.9x                         | 15.9x    |
| tsv-wasm-internal  | 4.39       | 64  | 0.23    | 0.23    | 0.23    | 0.23    | 0.23    | 0.23    | 0.24    | 9.80x                         | 9.79x    |
| oxc-parser         | 0.48       | 23  | 2.10    | 2.13    | 2.14    | 2.14    | 2.15    | 2.06    | 2.16    | 1.06x                         | 1.06x    |
| oxc-parser-wasm    | 0.45       | 24  | 2.22    | 2.22    | 2.23    | 2.24    | 2.24    | 2.21    | 2.24    | 1.01x                         | 1.01x    |
| oxc-parser-raw     | 1.75       | 27  | 0.58    | 0.59    | 0.60    | 0.60    | 0.61    | 0.52    | 0.62    | 3.91x                         | 3.88x    |
| yuku-parser        | 1.83       | 25  | 0.55    | 0.55    | 0.55    | 0.56    | 0.56    | 0.54    | 0.56    | 4.08x                         | 4.07x    |
| yuku-parser-wasm   | 2.38       | 35  | 0.42    | 0.42    | 0.43    | 0.43    | 0.43    | 0.41    | 0.43    | 5.32x                         | 5.33x    |
| swc                | 0.37       | 24  | 2.67    | 2.68    | 2.68    | 2.68    | 2.68    | 2.66    | 2.68    | 0.84x                         | 0.84x    |

**Files (intersection):** 5698

**Throughput:** acorn-typescript 12.1 MB/s, tsv 19.5 MB/s, tsv-wasm 17.4 MB/s, tsv+locations 15.9 MB/s, tsv-wasm+locations 14.4 MB/s, tsv-internal 192.6 MB/s, tsv-wasm-internal 118.4 MB/s, oxc-parser 12.9 MB/s, oxc-parser-wasm 12.2 MB/s, oxc-parser-raw 47.2 MB/s, yuku-parser 49.3 MB/s, yuku-parser-wasm 64.3 MB/s, swc 10.1 MB/s

**Coverage:** acorn-typescript 5698/5700 (99%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), tsv+locations 5700/5700 (100%), tsv-wasm+locations 5700/5700 (100%), tsv-internal 5700/5700 (100%), tsv-wasm-internal 5700/5700 (100%), oxc-parser 5700/5700 (100%), oxc-parser-wasm 5700/5700 (100%), oxc-parser-raw 5700/5700 (100%), yuku-parser 5700/5700 (100%), yuku-parser-wasm 5700/5700 (100%), swc 5699/5700 (99%)

**Omitted from every row's timed set:** 2 of 5700 files, 0.1% of the group's bytes (0.0% of its files) — by row: acorn-typescript 2 (2 tool_limit); swc 1 (1 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.9x tsv-internal, tsv-wasm 6.8x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 22 | 24.16   | 24.36   | 24.44   | 24.44   | 24.45   | 23.91   | 24.45   | baseline              | baseline |
| tsv         | 1.99       | 28 | 0.50    | 0.50    | 0.50    | 0.51    | 0.51    | 0.50    | 0.51    | 48.2x                 | 48.2x    |
| tsv-wasm    | 1.07       | 24 | 0.94    | 0.94    | 0.94    | 0.94    | 0.94    | 0.93    | 0.94    | 25.8x                 | 25.8x    |
| oxfmt       | 0.86       | 24 | 1.17    | 1.17    | 1.19    | 1.19    | 1.19    | 1.15    | 1.19    | 20.7x                 | 20.7x    |
| biome-wasm  | 0.13       | 21 | 7.73    | 7.75    | 7.98    | 8.08    | 8.12    | 7.65    | 8.13    | 3.13x                 | 3.13x    |
| dprint-wasm | 0.20       | 24 | 5.03    | 5.05    | 5.05    | 5.06    | 5.06    | 4.99    | 5.06    | 4.81x                 | 4.80x    |

**Files (intersection):** 5699

**Throughput:** prettier 1.1 MB/s, tsv 53.8 MB/s, tsv-wasm 28.8 MB/s, oxfmt 23.1 MB/s, biome-wasm 3.5 MB/s, dprint-wasm 5.4 MB/s

**Coverage:** prettier 5700/5700 (100%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), oxfmt 5700/5700 (100%), biome-wasm 5699/5700 (99%), dprint-wasm 5700/5700 (100%)

**Omitted from every row's timed set:** 1 of 5700 files, 0.0% of the group's bytes (0.0% of its files) — by row: biome-wasm 1 (1 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 47.51      | 695  | 20.98    | 21.26    | 21.65    | 21.81    | 22.26    | 20.43    | 23.21    | baseline                     | baseline |
| tsv                | 27.13      | 370  | 36.81    | 37.58    | 38.44    | 39.40    | 42.61    | 35.53    | 43.73    | 0.57x                        | 0.57x    |
| tsv-wasm           | 24.36      | 337  | 41.00    | 41.69    | 43.62    | 44.92    | 46.49    | 39.78    | 47.25    | 0.51x                        | 0.51x    |
| tsv+locations      | 22.57      | 335  | 44.09    | 44.92    | 45.36    | 45.72    | 46.86    | 43.04    | 50.08    | 0.47x                        | 0.48x    |
| tsv-wasm+locations | 20.96      | 299  | 47.63    | 48.35    | 49.09    | 49.65    | 51.23    | 46.71    | 52.12    | 0.44x                        | 0.44x    |
| tsv-internal       | 200.38     | 2726 | 4.99     | 5.02     | 5.03     | 5.06     | 5.18     | 4.93     | 5.63     | 4.22x                        | 4.20x    |
| tsv-wasm-internal  | 121.57     | 1674 | 8.22     | 8.28     | 8.30     | 8.37     | 8.51     | 8.14     | 9.08     | 2.56x                        | 2.55x    |
| postcss            | 44.42      | 641  | 22.50    | 22.78    | 23.05    | 23.37    | 23.94    | 21.91    | 24.39    | 0.93x                        | 0.93x    |

**Files (intersection):** 192

**Throughput:** svelte/compiler 37.0 MB/s, tsv 21.1 MB/s, tsv-wasm 19.0 MB/s, tsv+locations 17.6 MB/s, tsv-wasm+locations 16.3 MB/s, tsv-internal 155.9 MB/s, tsv-wasm-internal 94.6 MB/s, postcss 34.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 7.4x tsv-internal, tsv-wasm 5.0x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.97       | 24   | 1025.77  | 1037.74  | 1046.09  | 1047.91  | 1052.09  | 1009.88  | 1053.26  | baseline              | baseline |
| tsv        | 113.95     | 1413 | 8.77     | 8.85     | 8.94     | 8.98     | 9.16     | 8.69     | 13.43    | 117.2x                | 116.9x   |
| tsv-wasm   | 69.32      | 921  | 14.47    | 14.58    | 14.69    | 14.80    | 15.06    | 14.13    | 15.61    | 71.3x                 | 70.9x    |
| oxfmt      | 28.74      | 430  | 34.70    | 35.27    | 35.94    | 36.33    | 37.53    | 32.95    | 37.79    | 29.6x                 | 29.6x    |
| biome-wasm | 5.55       | 78   | 183.60   | 186.42   | 188.77   | 190.26   | 192.10   | 170.28   | 193.98   | 5.70x                 | 5.59x    |
| malva-wasm | 13.69      | 190  | 73.06    | 73.34    | 73.74    | 73.94    | 74.45    | 72.54    | 74.74    | 14.1x                 | 14.0x    |

**Files (intersection):** 188

**Throughput:** prettier 0.7 MB/s, tsv 83.3 MB/s, tsv-wasm 50.7 MB/s, oxfmt 21.0 MB/s, biome-wasm 4.1 MB/s, malva-wasm 10.0 MB/s

**Coverage:** prettier 192/192 (100%), tsv 192/192 (100%), tsv-wasm 192/192 (100%), oxfmt 192/192 (100%), biome-wasm 189/192 (98%), malva-wasm 191/192 (99%)

**Omitted from every row's timed set:** 4 of 192 files, 6.0% of the group's bytes (2.1% of its files) — by row: biome-wasm 3 (1 tool_limit, 2 harvest_artifact); malva-wasm 1 (1 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.8 MB | 1.1 MB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.1 MB | 427.1 KB | 0.3x | 0.4x |
| tsv-wasm | 3.1 MB | 1.2 MB | — | — |
| biome (wasm) | 64.1 MB | 16.7 MB | 20.5x | 14.0x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.3x | 1.0x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.4x |
| yuku-parser (wasm) | 718.6 KB | 209.4 KB | 0.2x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.3x |
| markup_fmt (wasm) | 2.0 MB | 558.7 KB | 0.6x | 0.5x |
| tsv (ffi) | 4.8 MB | 2.4 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.6 MB | 1.7 MB | 0.7x | 0.7x |
| tsv parse (ffi) | 1.6 MB | 741.2 KB | 0.3x | 0.3x |
| tsv (napi) | 5.3 MB | 2.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.7 MB | 2.1x | 1.8x |
| oxc-parser (napi) | 2.2 MB | 906.0 KB | 0.4x | 0.3x |
| oxfmt (napi) | 9.1 MB | 3.8 MB | 1.7x | 1.4x |
| yuku-parser (napi) | 692.2 KB | 287.7 KB | 0.1x | 0.1x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 1.7x | 1.4x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 3.3x | 2.9x |
| swc (napi) | 10.7 MB | 10.3 MB | 2.0x | 3.9x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.7x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.7x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Of tsv's native rows, `tsv (ffi)` and `tsv (napi)` are the profile-guided build the native rows time; `tsv format (ffi)` and `tsv parse (ffi)` are plain builds, sized only. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (2329f) | **123.3x** prettier, **121.0x** oxfmt |
| format typescript (5699f) | **48.2x** prettier, **2.33x** oxfmt |
| format css (188f) | **117.2x** prettier, **3.96x** oxfmt |
| parse svelte (2331f) | **2.90x** svelte/compiler, **4.05x** rsvelte-parse |
| parse typescript (5698f) | **1.61x** acorn-typescript, **1.52x** oxc-parser, **0.41x** oxc-parser-raw, **0.40x** yuku-parser, **1.93x** swc |
| parse css (192f) | **0.57x** svelte/compiler, **0.61x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (2329f) | **59.0x** prettier, **12.1x** biome-wasm |
| format typescript (5699f) | **25.8x** prettier, **8.24x** biome-wasm, **5.38x** dprint-wasm |
| format css (188f) | **71.3x** prettier, **12.5x** biome-wasm, **5.06x** malva-wasm |
| parse svelte (2331f) | **2.50x** svelte/compiler |
| parse typescript (5698f) | **1.44x** acorn-typescript, **1.43x** oxc-parser-wasm, **0.27x** yuku-parser-wasm |
| parse css (192f) | **0.51x** svelte/compiler, **0.55x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. oxc-parser-raw is the same native parse as oxc-parser and hands JS the identical tree (byte parity graded per file), but crosses the boundary as a shared buffer that a generated JS deserializer builds the objects from instead of a JSON string — oxc’s undocumented `experimentalRawTransfer`. Deserialization is eager and complete before `parseSync` returns, so the row materializes the full tree like tsv’s default rows, by a transfer mechanism tsv does not ship. Native only, and absent under Bun, where oxc does not support raw transfer. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.32x, wasm 1.29x | typescript: native 1.23x, wasm 1.22x | css: native 1.20x, wasm 1.16x

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse/typescript/oxc-parser-raw | 3.9% | 4.2% | +4.1% | 6.8% | 27/28 |

## Skipped Files

73 files skipped, 74 unique file+error combinations — Svelte 66, TypeScript 3, CSS 4 files.

**Per-benchmark skip counts:**
- format/svelte: markup-fmt-wasm: 64
- format/css: biome-wasm: 3
- format/svelte: biome-wasm: 2
- parse/typescript: acorn-typescript: 2
- parse/typescript: swc: 1
- format/typescript: biome-wasm: 1
- format/css: malva-wasm: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
