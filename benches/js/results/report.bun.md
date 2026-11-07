# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.4

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-11-07T06:43:27.964Z — tsv 0.8.1 (872e4d3a)

**Corpus:** 2331 Svelte (4.4 MB), 5700 TypeScript (27.0 MB), 192 CSS (0.8 MB) — 8223 files, 32.2 MB total

**Corpus snapshot:** [fuzdev/corpora@dce40bd21](https://github.com/fuzdev/corpora/tree/dce40bd21d5e14fb13246db0ee730120f137cbb8) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 100% third-party by bytes (2331 files, 4.4 MB third-party · 0 files, 0.0 MB the author's own); TypeScript 100% third-party by bytes (5700 files, 27.0 MB third-party · 0 files, 0.0 MB the author's own); CSS 100% third-party by bytes (192 files, 0.8 MB third-party · 0 files, 0.0 MB the author's own)

**Native build:** the native tsv rows (N-API, under Node/Bun) measure the N-API addon's profile-guided build, trained on the corpus snapshot's `origin: own` collections — none of which this run’s corpus holds (zzz, fuz_app, fuz_blog, fuz_code, fuz_css, docs.fuz.dev, fuz_repos, fuz_template, fuz_ui, fuz_util, mdz, gro, svelte-docinfo, tsv.fuz.dev, ryanatkn.com, earbetter, cosmicplayground). The N-API addon's is the shipped addon's: the addon itself — parse on every file and format --jobs 1 over the tree through the @fuzdev/tsv loader’s cli.js, the type strip and import records in process — the training the published addon gets. WASM rows are not profile-guided.

**Sources:** ../corpora/collections/kit/packages/kit/src (316), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (153), ../corpora/collections/svelte.dev/packages/repl/src (55), ../corpora/collections/svelte.dev/packages/site-kit/src (75), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (656), ../corpora/collections/layercake/src (273), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (168), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/bits-ui/packages/bits-ui/src/lib (507), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/ui (461), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/hooks (2), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/lib (3), ../corpora/collections/shadcn-svelte/packages/cli/src (49), ../corpora/collections/shadcn-svelte/packages/primitives/src (49), ../corpora/collections/skeleton/packages/skeleton/src (27), ../corpora/collections/skeleton/packages/skeleton-common/src (27), ../corpora/collections/skeleton/packages/skeleton-svelte/src (449), ../corpora/collections/skeleton/packages/cli/src (37), ../corpora/collections/xyflow/packages/svelte/src/lib (129), ../corpora/collections/xyflow/packages/svelte/src/styles (2), ../corpora/collections/xyflow/packages/system/src (49), ../corpora/collections/threlte/packages/core/src/lib (62), ../corpora/collections/threlte/packages/extras/src/lib (278), ../corpora/collections/threlte/packages/flex/src/lib (20), ../corpora/collections/threlte/packages/rapier/src/lib (42), ../corpora/collections/threlte/packages/studio/src/lib (105), ../corpora/collections/threlte/packages/theatre/src/lib (40), ../corpora/collections/threlte/packages/xr/src/lib (55), ../corpora/collections/runed/packages/runed/src/lib (105), ../corpora/collections/daisyui/packages/daisyui/src (72), ../corpora/collections/open-props/src/extra (16), ../corpora/collections/open-props/src/props.highlights.css (1), ../corpora/collections/open-props/src/props.layouts.css (1), ../corpora/collections/open-props/src/props.shadows.dark.css (1), ../corpora/collections/open-props/src/props.shadows.light.css (1), ../corpora/collections/open-props/src/props.supports.css (1), ../corpora/collections/open-props/docsite/index.css (1), ../corpora/collections/open-props/docsite/color-copy.css (1), ../corpora/collections/open-props/docsite/palette-playground.css (1), ../corpora/collections/open-props/docsite/syntax-highlight.css (1), ../corpora/collections/open-props/docsite/js (11), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/addons/docs/src/csf-plugin (6), ../corpora/collections/storybook/code/core/src (1361), ../corpora/collections/storybook/code/lib/cli-storybook/src (151), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (81), ../corpora/collections/storybook/code/lib/eslint-plugin/src (49), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (9)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.8% apart at the median, 2.3% at the 95th percentile and 9.5% at most (138 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.23       | 24  | 814.82   | 832.11   | 838.63   | 839.31   | 840.09   | 785.74   | 840.30   | baseline                     | baseline |
| tsv                         | 6.95       | 105 | 144.13   | 145.42   | 146.70   | 147.22   | 148.35   | 139.59   | 149.24   | 5.67x                        | 5.65x    |
| tsv-wasm                    | 5.36       | 82  | 186.59   | 188.06   | 189.75   | 190.60   | 192.52   | 182.77   | 193.46   | 4.37x                        | 4.37x    |
| tsv+locations               | 4.32       | 66  | 231.37   | 234.23   | 236.30   | 237.13   | 239.31   | 224.18   | 241.26   | 3.52x                        | 3.52x    |
| tsv-wasm+locations          | 3.66       | 57  | 272.89   | 274.66   | 277.33   | 278.46   | 280.15   | 266.45   | 280.72   | 2.99x                        | 2.99x    |
| tsv-internal                | 40.70      | 576 | 24.55    | 24.70    | 24.98    | 25.13    | 25.38    | 24.24    | 28.82    | 33.2x                        | 33.2x    |
| tsv-wasm-internal           | 21.75      | 287 | 46.01    | 46.15    | 46.55    | 46.71    | 47.30    | 45.53    | 48.08    | 17.7x                        | 17.7x    |
| rsvelte-parse               | 1.23       | 24  | 811.57   | 813.52   | 816.01   | 816.21   | 816.91   | 806.46   | 817.11   | 1.01x                        | 1.00x    |
| rsvelte-parse-skip-expr-loc | 2.03       | 33  | 492.61   | 494.06   | 495.23   | 496.07   | 497.94   | 488.60   | 498.31   | 1.66x                        | 1.65x    |

**Files (intersection):** 2331

**Throughput:** svelte/compiler 5.4 MB/s, tsv 30.5 MB/s, tsv-wasm 23.5 MB/s, tsv+locations 18.9 MB/s, tsv-wasm+locations 16.1 MB/s, tsv-internal 178.3 MB/s, tsv-wasm-internal 95.3 MB/s, rsvelte-parse 5.4 MB/s, rsvelte-parse-skip-expr-loc 8.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 5.9x tsv-internal, tsv-wasm 4.1x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.11       | 22  | 9.19    | 9.27    | 9.37    | 9.42    | 9.53    | 9.03    | 9.56    | baseline              | baseline |
| tsv        | 11.12      | 154 | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 102.2x                | 102.2x   |
| tsv-wasm   | 5.30       | 79  | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 0.19    | 48.8x                 | 48.7x    |
| oxfmt      | 0.11       | 23  | 8.92    | 9.08    | 9.14    | 9.27    | 9.33    | 8.66    | 9.34    | 1.03x                 | 1.03x    |
| biome-wasm | 0.50       | 21  | 2.01    | 2.02    | 2.03    | 2.03    | 2.04    | 1.91    | 2.05    | 4.62x                 | 4.58x    |

**Files (intersection):** 2329

**Throughput:** prettier 0.5 MB/s, tsv 48.6 MB/s, tsv-wasm 23.2 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 2331/2331 (100%), tsv 2331/2331 (100%), tsv-wasm 2331/2331 (100%), oxfmt 2331/2331 (100%), biome-wasm 2329/2331 (99%)

**Omitted from every row's timed set:** 2 of 2331 files, 0.1% of the group's bytes (0.1% of its files) — by row: biome-wasm 2 (2 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 2267/2331 (97%) — it rejects valid Svelte in the real corpus (a quoted `style` attribute whose value is no declaration list, some inline arrow-function event handlers), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 2331/2331 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript   | 0.52       | 24  | 1934.49  | 1949.71  | 1956.28  | 1967.18  | 1975.36  | 1870.24  | 1977.24  | baseline                      | baseline |
| tsv                | 1.50       | 24  | 670.14   | 671.81   | 673.91   | 674.03   | 675.66   | 656.21   | 676.14   | 2.89x                         | 2.89x    |
| tsv-wasm           | 1.18       | 24  | 836.50   | 895.32   | 898.27   | 899.95   | 900.87   | 817.26   | 901.08   | 2.27x                         | 2.31x    |
| tsv+locations      | 1.02       | 24  | 976.55   | 1003.61  | 1008.52  | 1011.41  | 1014.32  | 966.17   | 1015.10  | 1.96x                         | 1.98x    |
| tsv-wasm+locations | 0.87       | 22  | 1144.81  | 1161.33  | 1165.05  | 1166.28  | 1172.22  | 1128.68  | 1173.96  | 1.68x                         | 1.69x    |
| tsv-internal       | 8.35       | 126 | 119.73   | 120.14   | 120.45   | 120.59   | 120.84   | 118.84   | 120.95   | 16.1x                         | 16.2x    |
| tsv-wasm-internal  | 4.57       | 68  | 218.91   | 219.32   | 219.75   | 219.92   | 221.27   | 217.93   | 222.83   | 8.80x                         | 8.84x    |
| oxc-parser         | 1.00       | 24  | 1000.50  | 1002.25  | 1005.25  | 1006.96  | 1008.32  | 987.17   | 1008.66  | 1.93x                         | 1.93x    |
| oxc-parser-wasm    | 0.68       | 24  | 1463.82  | 1510.00  | 1534.51  | 1537.84  | 1552.60  | 1406.99  | 1556.87  | 1.31x                         | 1.32x    |
| yuku-parser        | 2.07       | 30  | 483.93   | 486.30   | 488.61   | 489.92   | 491.82   | 472.77   | 492.54   | 4.00x                         | 4.00x    |
| yuku-parser-wasm   | 2.76       | 42  | 361.94   | 363.79   | 366.13   | 368.48   | 369.20   | 353.53   | 369.40   | 5.33x                         | 5.34x    |
| swc                | 0.63       | 21  | 1575.47  | 1585.23  | 1586.61  | 1589.64  | 1590.34  | 1569.20  | 1590.40  | 1.22x                         | 1.23x    |

**Files (intersection):** 5698

**Throughput:** acorn-typescript 14.0 MB/s, tsv 40.4 MB/s, tsv-wasm 31.7 MB/s, tsv+locations 27.4 MB/s, tsv-wasm+locations 23.5 MB/s, tsv-internal 225.3 MB/s, tsv-wasm-internal 123.3 MB/s, oxc-parser 27.0 MB/s, oxc-parser-wasm 18.4 MB/s, yuku-parser 55.9 MB/s, yuku-parser-wasm 74.6 MB/s, swc 17.1 MB/s

**Coverage:** acorn-typescript 5698/5700 (99%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), tsv+locations 5700/5700 (100%), tsv-wasm+locations 5700/5700 (100%), tsv-internal 5700/5700 (100%), tsv-wasm-internal 5700/5700 (100%), oxc-parser 5700/5700 (100%), oxc-parser-wasm 5700/5700 (100%), yuku-parser 5700/5700 (100%), yuku-parser-wasm 5700/5700 (100%), swc 5699/5700 (99%)

**Omitted from every row's timed set:** 2 of 5700 files, 0.1% of the group's bytes (0.0% of its files) — by row: acorn-typescript 2 (2 tool_limit); swc 1 (1 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 5.6x tsv-internal, tsv-wasm 3.9x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.05       | 24 | 21.01   | 21.10   | 21.21   | 21.29   | 21.31   | 20.66   | 21.31   | baseline              | baseline |
| tsv         | 2.06       | 33 | 0.48    | 0.49    | 0.49    | 0.49    | 0.49    | 0.48    | 0.49    | 43.3x                 | 43.4x    |
| tsv-wasm    | 1.08       | 24 | 0.92    | 0.93    | 0.93    | 0.93    | 0.93    | 0.92    | 0.93    | 22.7x                 | 22.7x    |
| oxfmt       | 0.89       | 24 | 1.13    | 1.14    | 1.15    | 1.15    | 1.15    | 1.11    | 1.15    | 18.6x                 | 18.6x    |
| biome-wasm  | 0.15       | 24 | 6.86    | 6.89    | 6.91    | 6.96    | 6.97    | 6.78    | 6.97    | 3.06x                 | 3.06x    |
| dprint-wasm | 0.20       | 24 | 4.90    | 4.91    | 4.91    | 4.92    | 4.92    | 4.87    | 4.92    | 4.29x                 | 4.29x    |

**Files (intersection):** 5699

**Throughput:** prettier 1.3 MB/s, tsv 55.7 MB/s, tsv-wasm 29.2 MB/s, oxfmt 23.9 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.5 MB/s

**Coverage:** prettier 5700/5700 (100%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), oxfmt 5700/5700 (100%), biome-wasm 5699/5700 (99%), dprint-wasm 5700/5700 (100%)

**Omitted from every row's timed set:** 1 of 5700 files, 0.0% of the group's bytes (0.0% of its files) — by row: biome-wasm 1 (1 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 27.19      | 408  | 36.78    | 37.36    | 37.88    | 38.23    | 39.12    | 34.43    | 40.02    | baseline                     | baseline |
| tsv                | 53.52      | 796  | 18.66    | 18.87    | 19.06    | 19.19    | 19.49    | 18.09    | 19.87    | 1.97x                        | 1.97x    |
| tsv-wasm           | 46.71      | 691  | 21.34    | 21.75    | 22.28    | 22.67    | 23.35    | 20.24    | 24.69    | 1.72x                        | 1.72x    |
| tsv+locations      | 36.92      | 547  | 27.06    | 27.36    | 27.81    | 28.07    | 28.60    | 25.63    | 30.66    | 1.36x                        | 1.36x    |
| tsv-wasm+locations | 33.89      | 509  | 29.47    | 29.89    | 30.32    | 30.55    | 31.04    | 28.20    | 31.66    | 1.25x                        | 1.25x    |
| tsv-internal       | 234.03     | 3244 | 4.27     | 4.28     | 4.31     | 4.33     | 4.49     | 4.24     | 5.07     | 8.61x                        | 8.61x    |
| tsv-wasm-internal  | 144.44     | 1925 | 6.93     | 6.95     | 6.98     | 7.03     | 7.15     | 6.86     | 11.19    | 5.31x                        | 5.31x    |
| postcss            | 35.55      | 533  | 27.93    | 28.92    | 29.84    | 30.33    | 30.92    | 25.36    | 32.18    | 1.31x                        | 1.32x    |

**Files (intersection):** 192

**Throughput:** svelte/compiler 21.2 MB/s, tsv 41.6 MB/s, tsv-wasm 36.3 MB/s, tsv+locations 28.7 MB/s, tsv-wasm+locations 26.4 MB/s, tsv-internal 182.1 MB/s, tsv-wasm-internal 112.4 MB/s, postcss 27.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 4.4x tsv-internal, tsv-wasm 3.1x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.18       | 24   | 849.88   | 855.62   | 861.36   | 863.32   | 864.88   | 825.89   | 865.31   | baseline              | baseline |
| tsv        | 126.95     | 1586 | 7.87     | 7.99     | 8.10     | 8.16     | 8.53     | 7.77     | 11.83    | 107.7x                | 108.0x   |
| tsv-wasm   | 79.34      | 1096 | 12.61    | 12.72    | 12.81    | 12.92    | 13.20    | 12.37    | 13.65    | 67.3x                 | 67.4x    |
| oxfmt      | 29.64      | 443  | 33.79    | 34.40    | 34.80    | 35.10    | 35.80    | 31.41    | 36.61    | 25.2x                 | 25.2x    |
| biome-wasm | 6.68       | 95   | 149.00   | 153.23   | 154.97   | 157.79   | 163.19   | 142.43   | 163.59   | 5.67x                 | 5.70x    |
| malva-wasm | 12.17      | 173  | 82.31    | 82.74    | 83.35    | 83.71    | 84.31    | 80.42    | 86.84    | 10.3x                 | 10.3x    |

**Files (intersection):** 188

**Throughput:** prettier 0.9 MB/s, tsv 92.8 MB/s, tsv-wasm 58.0 MB/s, oxfmt 21.7 MB/s, biome-wasm 4.9 MB/s, malva-wasm 8.9 MB/s

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
| format svelte (2329f) | **102.2x** prettier, **99.2x** oxfmt |
| format typescript (5699f) | **43.3x** prettier, **2.33x** oxfmt |
| format css (188f) | **107.7x** prettier, **4.28x** oxfmt |
| parse svelte (2331f) | **5.67x** svelte/compiler, **5.64x** rsvelte-parse |
| parse typescript (5698f) | **2.89x** acorn-typescript, **1.50x** oxc-parser, **0.72x** yuku-parser, **2.36x** swc |
| parse css (192f) | **1.97x** svelte/compiler, **1.51x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (2329f) | **48.8x** prettier, **10.6x** biome-wasm |
| format typescript (5699f) | **22.7x** prettier, **7.42x** biome-wasm, **5.29x** dprint-wasm |
| format css (188f) | **67.3x** prettier, **11.9x** biome-wasm, **6.52x** malva-wasm |
| parse svelte (2331f) | **4.37x** svelte/compiler |
| parse typescript (5698f) | **2.27x** acorn-typescript, **1.73x** oxc-parser-wasm, **0.43x** yuku-parser-wasm |
| parse css (192f) | **1.72x** svelte/compiler, **1.31x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.61x, wasm 1.46x | typescript: native 1.48x, wasm 1.35x | css: native 1.45x, wasm 1.38x

## Unstable Rows

2 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse/typescript/tsv-wasm | 4.0% | 4.0% | +0.5% | 9.5% | 24/24 |
| parse/typescript/oxc-parser-wasm | 3.1% | 3.1% | +5.6% | 0.5% | 24/24 |

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
