# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.8

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-11-07T04:38:21.464Z — tsv 0.8.1 (872e4d3a)

**Corpus:** 2331 Svelte (4.4 MB), 5700 TypeScript (27.0 MB), 192 CSS (0.8 MB) — 8223 files, 32.2 MB total

**Corpus snapshot:** [fuzdev/corpora@dce40bd21](https://github.com/fuzdev/corpora/tree/dce40bd21d5e14fb13246db0ee730120f137cbb8) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 100% third-party by bytes (2331 files, 4.4 MB third-party · 0 files, 0.0 MB the author's own); TypeScript 100% third-party by bytes (5700 files, 27.0 MB third-party · 0 files, 0.0 MB the author's own); CSS 100% third-party by bytes (192 files, 0.8 MB third-party · 0 files, 0.0 MB the author's own)

**Native build:** the native tsv rows (FFI, under Deno) measure the FFI library's profile-guided build, trained on the corpus snapshot's `origin: own` collections — none of which this run’s corpus holds (zzz, fuz_app, fuz_blog, fuz_code, fuz_css, docs.fuz.dev, fuz_repos, fuz_template, fuz_ui, fuz_util, mdz, gro, svelte-docinfo, tsv.fuz.dev, ryanatkn.com, earbetter, cosmicplayground). The FFI library's profile is its own: the library itself, in process under Deno through the bench’s own FFI wrapper — parse, parse_internal and format on every file. WASM rows are not profile-guided.

**Sources:** ../corpora/collections/kit/packages/kit/src (316), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (153), ../corpora/collections/svelte.dev/packages/repl/src (55), ../corpora/collections/svelte.dev/packages/site-kit/src (75), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (656), ../corpora/collections/layercake/src (273), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (168), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/bits-ui/packages/bits-ui/src/lib (507), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/ui (461), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/hooks (2), ../corpora/collections/shadcn-svelte/docs/src/lib/registry/lib (3), ../corpora/collections/shadcn-svelte/packages/cli/src (49), ../corpora/collections/shadcn-svelte/packages/primitives/src (49), ../corpora/collections/skeleton/packages/skeleton/src (27), ../corpora/collections/skeleton/packages/skeleton-common/src (27), ../corpora/collections/skeleton/packages/skeleton-svelte/src (449), ../corpora/collections/skeleton/packages/cli/src (37), ../corpora/collections/xyflow/packages/svelte/src/lib (129), ../corpora/collections/xyflow/packages/svelte/src/styles (2), ../corpora/collections/xyflow/packages/system/src (49), ../corpora/collections/threlte/packages/core/src/lib (62), ../corpora/collections/threlte/packages/extras/src/lib (278), ../corpora/collections/threlte/packages/flex/src/lib (20), ../corpora/collections/threlte/packages/rapier/src/lib (42), ../corpora/collections/threlte/packages/studio/src/lib (105), ../corpora/collections/threlte/packages/theatre/src/lib (40), ../corpora/collections/threlte/packages/xr/src/lib (55), ../corpora/collections/runed/packages/runed/src/lib (105), ../corpora/collections/daisyui/packages/daisyui/src (72), ../corpora/collections/open-props/src/extra (16), ../corpora/collections/open-props/src/props.highlights.css (1), ../corpora/collections/open-props/src/props.layouts.css (1), ../corpora/collections/open-props/src/props.shadows.dark.css (1), ../corpora/collections/open-props/src/props.shadows.light.css (1), ../corpora/collections/open-props/src/props.supports.css (1), ../corpora/collections/open-props/docsite/index.css (1), ../corpora/collections/open-props/docsite/color-copy.css (1), ../corpora/collections/open-props/docsite/palette-playground.css (1), ../corpora/collections/open-props/docsite/syntax-highlight.css (1), ../corpora/collections/open-props/docsite/js (11), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/addons/docs/src/csf-plugin (6), ../corpora/collections/storybook/code/core/src (1361), ../corpora/collections/storybook/code/lib/cli-storybook/src (151), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (81), ../corpora/collections/storybook/code/lib/eslint-plugin/src (49), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (9)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 2.4% at the 95th percentile and 6.0% at most (141 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.45       | 24  | 691.29   | 695.51   | 698.48   | 701.00   | 702.09   | 681.84   | 702.30   | baseline                     | baseline |
| tsv                         | 4.00       | 61  | 250.67   | 251.71   | 252.33   | 252.73   | 252.76   | 246.60   | 252.79   | 2.76x                        | 2.76x    |
| tsv-wasm                    | 3.10       | 48  | 322.38   | 324.38   | 325.74   | 325.92   | 326.30   | 319.10   | 326.36   | 2.14x                        | 2.14x    |
| tsv+locations               | 2.81       | 39  | 356.66   | 357.90   | 360.02   | 369.97   | 375.84   | 353.05   | 376.37   | 1.94x                        | 1.94x    |
| tsv-wasm+locations          | 2.30       | 32  | 435.61   | 437.43   | 439.02   | 452.37   | 454.19   | 430.82   | 455.01   | 1.59x                        | 1.59x    |
| tsv-internal                | 37.69      | 499 | 26.51    | 26.72    | 26.97    | 27.10    | 27.48    | 26.30    | 28.84    | 26.0x                        | 26.1x    |
| tsv-wasm-internal           | 18.38      | 245 | 54.44    | 54.61    | 54.89    | 55.14    | 55.43    | 53.91    | 55.47    | 12.7x                        | 12.7x    |
| rsvelte-parse               | 0.95       | 23  | 1049.63  | 1057.44  | 1059.76  | 1060.32  | 1060.57  | 1043.68  | 1060.63  | 0.66x                        | 0.66x    |
| rsvelte-parse-skip-expr-loc | 1.62       | 26  | 615.87   | 618.91   | 620.35   | 620.59   | 621.81   | 610.21   | 622.23   | 1.12x                        | 1.12x    |

**Files (intersection):** 2331

**Throughput:** svelte/compiler 6.3 MB/s, tsv 17.5 MB/s, tsv-wasm 13.6 MB/s, tsv+locations 12.3 MB/s, tsv-wasm+locations 10.1 MB/s, tsv-internal 165.1 MB/s, tsv-wasm-internal 80.5 MB/s, rsvelte-parse 4.2 MB/s, rsvelte-parse-skip-expr-loc 7.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.4x tsv-internal, tsv-wasm 5.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.10       | 24  | 10.46   | 10.55   | 10.61   | 10.65   | 10.66   | 10.35   | 10.66   | baseline              | baseline |
| tsv        | 10.64      | 157 | 0.09    | 0.09    | 0.09    | 0.10    | 0.10    | 0.09    | 0.10    | 111.6x                | 111.5x   |
| tsv-wasm   | 4.57       | 69  | 0.22    | 0.22    | 0.22    | 0.22    | 0.22    | 0.22    | 0.23    | 47.9x                 | 47.6x    |
| oxfmt      | 0.09       | 21  | 10.55   | 10.67   | 10.76   | 10.77   | 10.77   | 10.46   | 10.78   | 0.99x                 | 0.99x    |
| biome-wasm | 0.49       | 24  | 2.05    | 2.05    | 2.07    | 2.07    | 2.08    | 2.02    | 2.08    | 5.12x                 | 5.11x    |

**Files (intersection):** 2329

**Throughput:** prettier 0.4 MB/s, tsv 46.6 MB/s, tsv-wasm 20.0 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage:** prettier 2331/2331 (100%), tsv 2331/2331 (100%), tsv-wasm 2331/2331 (100%), oxfmt 2331/2331 (100%), biome-wasm 2329/2331 (99%)

**Omitted from every row's timed set:** 2 of 2331 files, 0.1% of the group's bytes (0.1% of its files) — by row: biome-wasm 2 (2 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 2267/2331 (97%) — it rejects valid Svelte in the real corpus (a quoted `style` attribute whose value is no declaration list, some inline arrow-function event handlers), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 2331/2331 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.59       | 24  | 1.71    | 1.72    | 1.72    | 1.72    | 1.72    | 1.69    | 1.72    | baseline                      | baseline |
| tsv                | 0.84       | 23  | 1.20    | 1.20    | 1.20    | 1.21    | 1.21    | 1.19    | 1.21    | 1.43x                         | 1.43x    |
| tsv-wasm           | 0.67       | 23  | 1.50    | 1.51    | 1.51    | 1.51    | 1.51    | 1.49    | 1.51    | 1.14x                         | 1.13x    |
| tsv+locations      | 0.66       | 22  | 1.53    | 1.53    | 1.53    | 1.53    | 1.53    | 1.52    | 1.53    | 1.12x                         | 1.12x    |
| tsv-wasm+locations | 0.54       | 24  | 1.84    | 1.84    | 1.85    | 1.85    | 1.85    | 1.83    | 1.85    | 0.93x                         | 0.93x    |
| tsv-internal       | 7.83       | 117 | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 13.4x                         | 13.4x    |
| tsv-wasm-internal  | 3.77       | 57  | 0.26    | 0.27    | 0.27    | 0.27    | 0.27    | 0.26    | 0.27    | 6.44x                         | 6.44x    |
| oxc-parser         | 0.53       | 24  | 1.89    | 1.89    | 1.90    | 1.90    | 1.90    | 1.87    | 1.90    | 0.90x                         | 0.90x    |
| oxc-parser-wasm    | 0.47       | 24  | 2.11    | 2.12    | 2.13    | 2.13    | 2.14    | 2.09    | 2.14    | 0.81x                         | 0.81x    |
| oxc-parser-raw     | 1.86       | 21  | 0.54    | 0.54    | 0.54    | 0.54    | 0.54    | 0.53    | 0.54    | 3.18x                         | 3.19x    |
| yuku-parser        | 1.48       | 21  | 0.68    | 0.69    | 0.70    | 0.70    | 0.70    | 0.63    | 0.70    | 2.53x                         | 2.50x    |
| yuku-parser-wasm   | 1.73       | 20  | 0.58    | 0.59    | 0.59    | 0.59    | 0.59    | 0.53    | 0.59    | 2.96x                         | 2.92x    |
| swc                | 0.41       | 24  | 2.43    | 2.44    | 2.44    | 2.44    | 2.44    | 2.41    | 2.44    | 0.70x                         | 0.70x    |

**Files (intersection):** 5698

**Throughput:** acorn-typescript 15.8 MB/s, tsv 22.6 MB/s, tsv-wasm 18.0 MB/s, tsv+locations 17.7 MB/s, tsv-wasm+locations 14.7 MB/s, tsv-internal 211.2 MB/s, tsv-wasm-internal 101.8 MB/s, oxc-parser 14.3 MB/s, oxc-parser-wasm 12.8 MB/s, oxc-parser-raw 50.3 MB/s, yuku-parser 40.0 MB/s, yuku-parser-wasm 46.7 MB/s, swc 11.1 MB/s

**Coverage:** acorn-typescript 5698/5700 (99%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), tsv+locations 5700/5700 (100%), tsv-wasm+locations 5700/5700 (100%), tsv-internal 5700/5700 (100%), tsv-wasm-internal 5700/5700 (100%), oxc-parser 5700/5700 (100%), oxc-parser-wasm 5700/5700 (100%), oxc-parser-raw 5700/5700 (100%), yuku-parser 5700/5700 (100%), yuku-parser-wasm 5700/5700 (100%), swc 5699/5700 (99%)

**Omitted from every row's timed set:** 2 of 5700 files, 0.1% of the group's bytes (0.0% of its files) — by row: acorn-typescript 2 (2 tool_limit); swc 1 (1 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 9.4x tsv-internal, tsv-wasm 5.7x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.05       | 22 | 21.66   | 21.73   | 21.81   | 21.83   | 21.85   | 20.84   | 21.86   | baseline              | baseline |
| tsv         | 2.04       | 32 | 0.49    | 0.49    | 0.49    | 0.49    | 0.49    | 0.49    | 0.49    | 43.8x                 | 44.2x    |
| tsv-wasm    | 0.93       | 24 | 1.09    | 1.09    | 1.09    | 1.09    | 1.09    | 1.07    | 1.09    | 19.9x                 | 20.0x    |
| oxfmt       | 0.86       | 21 | 1.16    | 1.18    | 1.19    | 1.19    | 1.20    | 1.14    | 1.20    | 18.4x                 | 18.6x    |
| biome-wasm  | 0.13       | 23 | 7.57    | 7.59    | 7.60    | 7.61    | 7.61    | 7.52    | 7.62    | 2.84x                 | 2.86x    |
| dprint-wasm | 0.17       | 24 | 5.72    | 5.72    | 5.73    | 5.73    | 5.73    | 5.70    | 5.73    | 3.75x                 | 3.79x    |

**Files (intersection):** 5699

**Throughput:** prettier 1.3 MB/s, tsv 55.0 MB/s, tsv-wasm 25.0 MB/s, oxfmt 23.2 MB/s, biome-wasm 3.6 MB/s, dprint-wasm 4.7 MB/s

**Coverage:** prettier 5700/5700 (100%), tsv 5700/5700 (100%), tsv-wasm 5700/5700 (100%), oxfmt 5700/5700 (100%), biome-wasm 5699/5700 (99%), dprint-wasm 5700/5700 (100%)

**Omitted from every row's timed set:** 1 of 5700 files, 0.0% of the group's bytes (0.0% of its files) — by row: biome-wasm 1 (1 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 50.41      | 754  | 19.77    | 20.30    | 20.75    | 20.94    | 21.39    | 18.82    | 24.66    | baseline                     | baseline |
| tsv                | 31.51      | 445  | 31.72    | 31.96    | 32.40    | 32.64    | 33.16    | 31.04    | 33.40    | 0.63x                        | 0.62x    |
| tsv-wasm           | 24.75      | 347  | 40.35    | 40.63    | 41.10    | 41.25    | 41.85    | 39.92    | 43.63    | 0.49x                        | 0.49x    |
| tsv+locations      | 25.02      | 343  | 39.89    | 40.35    | 41.25    | 41.58    | 42.05    | 39.24    | 50.63    | 0.50x                        | 0.50x    |
| tsv-wasm+locations | 20.61      | 290  | 48.44    | 48.90    | 49.77    | 49.98    | 50.47    | 47.69    | 55.78    | 0.41x                        | 0.41x    |
| tsv-internal       | 215.55     | 2962 | 4.64     | 4.66     | 4.69     | 4.74     | 4.83     | 4.59     | 4.98     | 4.28x                        | 4.26x    |
| tsv-wasm-internal  | 108.27     | 1522 | 9.20     | 9.35     | 9.38     | 9.42     | 9.63     | 9.09     | 10.22    | 2.15x                        | 2.15x    |
| postcss            | 46.06      | 583  | 21.82    | 22.02    | 22.95    | 23.17    | 23.47    | 21.18    | 24.33    | 0.91x                        | 0.91x    |

**Files (intersection):** 192

**Throughput:** svelte/compiler 39.2 MB/s, tsv 24.5 MB/s, tsv-wasm 19.3 MB/s, tsv+locations 19.5 MB/s, tsv-wasm+locations 16.0 MB/s, tsv-internal 167.7 MB/s, tsv-wasm-internal 84.2 MB/s, postcss 35.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.8x tsv-internal, tsv-wasm 4.4x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.01       | 24   | 997.06   | 1002.78  | 1012.17  | 1013.51  | 1018.95  | 969.78   | 1020.54  | baseline              | baseline |
| tsv        | 122.11     | 1684 | 8.18     | 8.24     | 8.33     | 8.40     | 8.59     | 8.09     | 12.43    | 121.3x                | 121.9x   |
| tsv-wasm   | 60.92      | 860  | 16.41    | 16.48    | 16.57    | 16.66    | 16.92    | 16.25    | 17.13    | 60.5x                 | 60.8x    |
| oxfmt      | 29.14      | 433  | 34.38    | 34.78    | 35.24    | 35.52    | 36.51    | 32.52    | 37.28    | 28.9x                 | 29.0x    |
| biome-wasm | 5.74       | 73   | 174.18   | 175.61   | 178.69   | 180.32   | 181.95   | 172.05   | 182.67   | 5.70x                 | 5.72x    |
| malva-wasm | 12.46      | 181  | 80.21    | 80.50    | 80.93    | 81.19    | 82.48    | 79.74    | 84.50    | 12.4x                 | 12.4x    |

**Files (intersection):** 188

**Throughput:** prettier 0.7 MB/s, tsv 89.3 MB/s, tsv-wasm 44.6 MB/s, oxfmt 21.3 MB/s, biome-wasm 4.2 MB/s, malva-wasm 9.1 MB/s

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
| tsv (ffi) | 4.8 MB | 2.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.7 MB | 2.3x | 2.0x |
| tsv format (ffi) | 3.6 MB | 1.7 MB | 0.8x | 0.7x |
| tsv parse (ffi) | 1.6 MB | 741.2 KB | 0.3x | 0.3x |
| tsv (napi) | 5.3 MB | 2.6 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.2 MB | 906.0 KB | 0.4x | 0.4x |
| oxfmt (napi) | 9.1 MB | 3.8 MB | 1.9x | 1.6x |
| yuku-parser (napi) | 692.2 KB | 287.7 KB | 0.1x | 0.1x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 1.9x | 1.5x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 3.7x | 3.2x |
| swc (napi) | 10.7 MB | 10.3 MB | 2.2x | 4.3x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.7x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.7x | 0.5x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Of tsv's native rows, `tsv (ffi)` and `tsv (napi)` are the profile-guided build the native rows time; `tsv format (ffi)` and `tsv parse (ffi)` are plain builds, sized only. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (2329f) | **111.6x** prettier, **112.9x** oxfmt |
| format typescript (5699f) | **43.8x** prettier, **2.37x** oxfmt |
| format css (188f) | **121.3x** prettier, **4.19x** oxfmt |
| parse svelte (2331f) | **2.76x** svelte/compiler, **4.20x** rsvelte-parse |
| parse typescript (5698f) | **1.43x** acorn-typescript, **1.58x** oxc-parser, **0.45x** oxc-parser-raw, **0.56x** yuku-parser, **2.03x** swc |
| parse css (192f) | **0.63x** svelte/compiler, **0.68x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (2329f) | **47.9x** prettier, **9.35x** biome-wasm |
| format typescript (5699f) | **19.9x** prettier, **7.00x** biome-wasm, **5.29x** dprint-wasm |
| format css (188f) | **60.5x** prettier, **10.6x** biome-wasm, **4.89x** malva-wasm |
| parse svelte (2331f) | **2.14x** svelte/compiler |
| parse typescript (5698f) | **1.14x** acorn-typescript, **1.41x** oxc-parser-wasm, **0.38x** yuku-parser-wasm |
| parse css (192f) | **0.49x** svelte/compiler, **0.54x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. oxc-parser-raw is the same native parse as oxc-parser and hands JS the identical tree (byte parity graded per file), but crosses the boundary as a shared buffer that a generated JS deserializer builds the objects from instead of a JSON string — oxc’s undocumented `experimentalRawTransfer`. Deserialization is eager and complete before `parseSync` returns, so the row materializes the full tree like tsv’s default rows, by a transfer mechanism tsv does not ship. Native only, and absent under Bun, where oxc does not support raw transfer. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.42x, wasm 1.35x | typescript: native 1.27x, wasm 1.22x | css: native 1.26x, wasm 1.20x

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples a pass, where one deviant sweep is a real share of the row), a drift past 5% (within one pass, the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported; the row's worst pass is shown), or a pass spread past 5% (the row's slowest pass mean over its fastest — each pass is a fresh process, so this is a level that depends on the process the row was drawn in). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | pass spread | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: | ---: |
| parse/css/svelte/compiler | 3.1% | 3.3% | +0.2% | 6.0% | 754/758 |

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
