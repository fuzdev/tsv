# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.8

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-11-02T04:25:40.349Z — tsv 0.8.0 (83aa5582)

**Corpus:** 1898 Svelte (4.8 MB), 5691 TypeScript (34.1 MB), 71 CSS (0.5 MB) — 7660 files, 39.4 MB total

**Corpus snapshot:** [fuzdev/corpora@678f4942f](https://github.com/fuzdev/corpora/tree/678f4942f92ca0fab49a73db37f61546f34b470c) — the real-code sources are its collections, vendored at this commit

**By origin:** Svelte 57% third-party by bytes (1091 files, 2.8 MB third-party · 807 files, 2.1 MB the author's own); TypeScript 57% third-party by bytes (3895 files, 19.3 MB third-party · 1796 files, 14.8 MB the author's own); CSS 48% third-party by bytes (32 files, 0.2 MB third-party · 39 files, 0.3 MB the author's own)

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), ../corpora/collections/kit/packages/kit/src (227), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70), ../corpora/collections/flowbite-svelte/src/lib (359), ../corpora/collections/layerchart/packages/layerchart/src (532), ../corpora/collections/layercake/src (246), ../corpora/collections/svelte-ux/packages/svelte-ux/src (299), ../corpora/collections/svelte-maplibre/src (149), ../corpora/collections/language-tools/packages/language-server/src (88), ../corpora/collections/language-tools/packages/svelte2tsx/src (68), ../corpora/collections/language-tools/packages/typescript-plugin/src (26), ../corpora/collections/language-tools/packages/svelte-vscode/src (26), ../corpora/collections/language-tools/packages/svelte-check/src (6), ../corpora/collections/language-tools/packages/load-config/src (1), ../corpora/collections/typescript-parser/src/compiler/parser.ts (1), ../corpora/collections/storybook/code/core/src (1088), ../corpora/collections/storybook/code/lib/cli-storybook/src (111), ../corpora/collections/storybook/code/lib/codemod/src (11), ../corpora/collections/storybook/code/lib/core-webpack/src (12), ../corpora/collections/storybook/code/lib/create-storybook/src (75), ../corpora/collections/storybook/code/lib/csf-plugin/src (4), ../corpora/collections/storybook/code/lib/eslint-plugin/src (47), ../corpora/collections/storybook/code/lib/react-dom-shim/src (1), ../corpora/collections/continue/core (602), ../corpora/collections/continue/extensions/cli (349), benches/js/.cache/svelte_styles (24)

**Versions:** svelte@5.57.3, acorn@8.19.0, acorn-typescript@1.0.13, prettier@3.9.10, prettier-plugin-svelte@4.1.1, oxc-parser@0.156.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.75.0, yuku-parser@0.19.0, @biomejs/wasm-bundler@2.6.4, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, dprint-plugin-markup@0.27.6, postcss@8.5.32, @rsvelte/fmt@0.7.25, @rsvelte/vite-plugin-svelte-native@0.3.17 (targets svelte@5.57.1), @swc/core@1.16.21

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

**Isolation:** every row is timed in a process of its own that loads that row’s engine and nothing else measured, 3 times over (passes), each pass taking a group’s rows in a different order; a row’s statistics pool its passes. Two fresh processes of the same row sat 0.5% apart at the median, 1.7% at the 95th percentile and 4.4% at most (141 pass pairs) — a ratio inside that is not a difference this run measured.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.39       | 24  | 719.81   | 725.91   | 733.92   | 735.54   | 739.31   | 703.27   | 740.41   | baseline                     | baseline |
| tsv                         | 3.68       | 56  | 271.53   | 272.15   | 272.70   | 273.12   | 280.65   | 269.58   | 290.07   | 2.66x                        | 2.65x    |
| tsv-wasm                    | 2.96       | 44  | 337.18   | 338.86   | 339.67   | 340.28   | 340.70   | 334.62   | 340.93   | 2.14x                        | 2.13x    |
| tsv+locations               | 2.61       | 39  | 382.89   | 383.62   | 383.99   | 384.80   | 396.94   | 379.99   | 402.32   | 1.88x                        | 1.88x    |
| tsv-wasm+locations          | 2.20       | 31  | 452.97   | 455.71   | 457.09   | 464.19   | 469.78   | 451.30   | 471.55   | 1.59x                        | 1.59x    |
| tsv-internal                | 30.32      | 385 | 32.96    | 33.16    | 33.42    | 33.57    | 34.13    | 32.80    | 34.52    | 21.9x                        | 21.8x    |
| tsv-wasm-internal           | 16.95      | 236 | 58.98    | 59.16    | 59.50    | 59.78    | 60.20    | 58.55    | 60.44    | 12.2x                        | 12.2x    |
| rsvelte-parse               | 0.89       | 19  | 1129.54  | 1132.96  | 1142.87  | 1143.18  | 1145.61  | 1124.64  | 1146.33  | 0.64x                        | 0.64x    |
| rsvelte-parse-skip-expr-loc | 1.43       | 22  | 698.88   | 703.16   | 704.64   | 705.51   | 705.75   | 697.04   | 705.79   | 1.03x                        | 1.03x    |

**Files (intersection):** 1898

**Throughput:** svelte/compiler 6.7 MB/s, tsv 17.8 MB/s, tsv-wasm 14.3 MB/s, tsv+locations 12.6 MB/s, tsv-wasm+locations 10.7 MB/s, tsv-internal 146.6 MB/s, tsv-wasm-internal 81.9 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.2x tsv-internal, tsv-wasm 5.7x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n   | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.09       | 24  | 10.81   | 10.85   | 10.90   | 10.91   | 10.98   | 10.53   | 11.00   | baseline              | baseline |
| tsv        | 7.25       | 111 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 78.2x                 | 78.5x    |
| tsv-wasm   | 4.33       | 66  | 0.23    | 0.23    | 0.23    | 0.23    | 0.23    | 0.23    | 0.23    | 46.7x                 | 46.9x    |
| oxfmt      | 0.09       | 24  | 10.98   | 11.04   | 11.07   | 11.09   | 11.12   | 10.75   | 11.13   | 0.98x                 | 0.98x    |
| biome-wasm | 0.51       | 24  | 1.96    | 1.97    | 1.97    | 1.98    | 1.98    | 1.95    | 1.98    | 5.49x                 | 5.51x    |

**Files (intersection):** 1891

**Throughput:** prettier 0.4 MB/s, tsv 34.4 MB/s, tsv-wasm 20.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.4 MB/s

**Coverage:** prettier 1898/1898 (100%), tsv 1898/1898 (100%), tsv-wasm 1898/1898 (100%), oxfmt 1898/1898 (100%), biome-wasm 1891/1898 (99%)

**Omitted from every row's timed set:** 7 of 1898 files, 1.8% of the group's bytes (0.4% of its files) — by row: biome-wasm 7 (7 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** markup-fmt-wasm 1828/1898 (96%) — it rejects valid Svelte in the real corpus (markup inside a template literal in an attribute expression, `{…}` inside a quoted `style`), and a file any timed row rejects leaves every row’s timed set; rsvelte-fmt 1898/1898 (100%) — no in-process API, so a timed row would measure process spawn rather than format work. These are accept rates, not speeds.

## parse/typescript

| Task Name          | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| ------------------ | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript   | 0.51       | 23 | 1.97    | 1.98    | 1.99    | 2.00    | 2.01    | 1.95    | 2.02    | baseline                      | baseline |
| tsv                | 0.68       | 24 | 1.47    | 1.48    | 1.48    | 1.48    | 1.48    | 1.47    | 1.48    | 1.34x                         | 1.34x    |
| tsv-wasm           | 0.55       | 22 | 1.81    | 1.82    | 1.82    | 1.82    | 1.82    | 1.81    | 1.82    | 1.09x                         | 1.09x    |
| tsv+locations      | 0.54       | 24 | 1.87    | 1.87    | 1.88    | 1.88    | 1.88    | 1.86    | 1.88    | 1.05x                         | 1.05x    |
| tsv-wasm+locations | 0.45       | 24 | 2.21    | 2.22    | 2.23    | 2.23    | 2.23    | 2.19    | 2.24    | 0.89x                         | 0.89x    |
| tsv-internal       | 5.78       | 87 | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.18    | 11.4x                         | 11.4x    |
| tsv-wasm-internal  | 3.17       | 46 | 0.32    | 0.32    | 0.32    | 0.32    | 0.32    | 0.31    | 0.32    | 6.24x                         | 6.21x    |
| oxc-parser         | 0.44       | 22 | 2.28    | 2.29    | 2.30    | 2.31    | 2.31    | 2.27    | 2.31    | 0.86x                         | 0.86x    |
| oxc-parser-wasm    | 0.39       | 24 | 2.55    | 2.56    | 2.57    | 2.57    | 2.58    | 2.53    | 2.58    | 0.77x                         | 0.77x    |
| oxc-parser-raw     | 1.61       | 26 | 0.62    | 0.63    | 0.64    | 0.64    | 0.65    | 0.60    | 0.65    | 3.17x                         | 3.18x    |
| yuku-parser        | 1.28       | 19 | 0.78    | 0.79    | 0.84    | 0.84    | 0.85    | 0.77    | 0.85    | 2.53x                         | 2.52x    |
| yuku-parser-wasm   | 1.49       | 22 | 0.67    | 0.67    | 0.68    | 0.68    | 0.68    | 0.66    | 0.68    | 2.93x                         | 2.93x    |
| swc                | 0.33       | 23 | 2.99    | 3.00    | 3.00    | 3.00    | 3.00    | 2.97    | 3.00    | 0.66x                         | 0.66x    |

**Files (intersection):** 5688

**Throughput:** acorn-typescript 17.3 MB/s, tsv 23.1 MB/s, tsv-wasm 18.8 MB/s, tsv+locations 18.2 MB/s, tsv-wasm+locations 15.4 MB/s, tsv-internal 196.8 MB/s, tsv-wasm-internal 108.0 MB/s, oxc-parser 14.9 MB/s, oxc-parser-wasm 13.3 MB/s, oxc-parser-raw 54.9 MB/s, yuku-parser 43.7 MB/s, yuku-parser-wasm 50.8 MB/s, swc 11.4 MB/s

**Coverage:** acorn-typescript 5688/5691 (99%), tsv 5691/5691 (100%), tsv-wasm 5691/5691 (100%), tsv+locations 5691/5691 (100%), tsv-wasm+locations 5691/5691 (100%), tsv-internal 5691/5691 (100%), tsv-wasm-internal 5691/5691 (100%), oxc-parser 5689/5691 (99%), oxc-parser-wasm 5689/5691 (99%), oxc-parser-raw 5689/5691 (99%), yuku-parser 5689/5691 (99%), yuku-parser-wasm 5689/5691 (99%), swc 5688/5691 (99%)

**Omitted from every row's timed set:** 3 of 5691 files, 0.0% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); oxc-parser-raw 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 8.5x tsv-internal, tsv-wasm 5.7x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.04       | 24 | 25.96   | 26.12   | 26.38   | 26.41   | 26.52   | 24.89   | 26.56   | baseline              | baseline |
| tsv         | 1.35       | 24 | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 0.74    | 34.9x                 | 35.1x    |
| tsv-wasm    | 0.80       | 24 | 1.25    | 1.25    | 1.26    | 1.26    | 1.26    | 1.24    | 1.26    | 20.7x                 | 20.8x    |
| oxfmt       | 0.73       | 24 | 1.37    | 1.37    | 1.40    | 1.41    | 1.41    | 1.34    | 1.41    | 18.8x                 | 19.0x    |
| biome-wasm  | 0.12       | 24 | 8.66    | 8.70    | 8.72    | 8.72    | 8.72    | 8.58    | 8.73    | 2.98x                 | 3.00x    |
| dprint-wasm | 0.15       | 24 | 6.81    | 6.83    | 6.84    | 6.84    | 6.84    | 6.78    | 6.84    | 3.79x                 | 3.81x    |

**Files (intersection):** 5688

**Throughput:** prettier 1.3 MB/s, tsv 46.1 MB/s, tsv-wasm 27.3 MB/s, oxfmt 24.9 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 5691/5691 (100%), tsv 5691/5691 (100%), tsv-wasm 5691/5691 (100%), oxfmt 5689/5691 (99%), biome-wasm 5688/5691 (99%), dprint-wasm 5691/5691 (100%)

**Omitted from every row's timed set:** 3 of 5691 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name          | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ------------------ | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler    | 79.11      | 1173 | 12.60    | 12.85    | 13.06    | 13.19    | 13.53    | 12.14    | 15.45    | baseline                     | baseline |
| tsv                | 46.73      | 614  | 21.38    | 21.57    | 21.91    | 22.18    | 22.78    | 21.08    | 26.50    | 0.59x                        | 0.59x    |
| tsv-wasm           | 38.59      | 522  | 25.90    | 26.10    | 26.44    | 26.69    | 27.02    | 25.55    | 30.41    | 0.49x                        | 0.49x    |
| tsv+locations      | 36.31      | 537  | 27.48    | 28.33    | 28.50    | 28.64    | 33.21    | 26.65    | 34.28    | 0.46x                        | 0.46x    |
| tsv-wasm+locations | 31.30      | 464  | 31.90    | 32.51    | 32.87    | 33.09    | 35.87    | 31.12    | 38.52    | 0.40x                        | 0.40x    |
| tsv-internal       | 309.19     | 4210 | 3.23     | 3.24     | 3.26     | 3.29     | 3.39     | 3.20     | 4.44     | 3.91x                        | 3.90x    |
| tsv-wasm-internal  | 178.25     | 2517 | 5.62     | 5.65     | 5.68     | 5.71     | 5.83     | 5.52     | 6.06     | 2.25x                        | 2.24x    |
| postcss            | 65.29      | 969  | 15.27    | 15.55    | 15.82    | 15.94    | 16.55    | 14.61    | 17.47    | 0.83x                        | 0.83x    |

**Files (intersection):** 71

**Throughput:** svelte/compiler 39.6 MB/s, tsv 23.4 MB/s, tsv-wasm 19.3 MB/s, tsv+locations 18.2 MB/s, tsv-wasm+locations 15.7 MB/s, tsv-internal 155.0 MB/s, tsv-wasm-internal 89.3 MB/s, postcss 32.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv 6.6x tsv-internal, tsv-wasm 4.6x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.46       | 24   | 690.95   | 694.21   | 696.86   | 697.24   | 698.33   | 664.99   | 698.66   | baseline              | baseline |
| tsv        | 173.95     | 2332 | 5.75     | 5.78     | 5.85     | 5.90     | 6.04     | 5.67     | 10.53    | 119.5x                | 120.1x   |
| tsv-wasm   | 99.69      | 1331 | 10.03    | 10.10    | 10.16    | 10.24    | 10.49    | 9.91     | 11.88    | 68.5x                 | 68.9x    |
| oxfmt      | 46.16      | 684  | 21.67    | 21.99    | 22.32    | 22.64    | 23.55    | 20.10    | 24.65    | 31.7x                 | 31.9x    |
| biome-wasm | 9.79       | 134  | 102.18   | 103.14   | 104.23   | 105.21   | 114.42   | 99.88    | 117.90   | 6.73x                 | 6.76x    |
| malva-wasm | 15.21      | 200  | 65.75    | 65.90    | 66.32    | 66.52    | 67.55    | 65.31    | 68.42    | 10.5x                 | 10.5x    |

**Files (intersection):** 69

**Throughput:** prettier 0.7 MB/s, tsv 80.2 MB/s, tsv-wasm 45.9 MB/s, oxfmt 21.3 MB/s, biome-wasm 4.5 MB/s, malva-wasm 7.0 MB/s

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
| tsv (ffi) | 3.9 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.7 MB | 2.9x | 2.6x |
| tsv format (ffi) | 3.6 MB | 1.7 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 739.0 KB | 0.4x | 0.4x |
| tsv (napi) | 4.3 MB | 2.0 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.2 MB | 906.0 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.8 MB | 2.4x | 2.1x |
| yuku-parser (napi) | 692.2 KB | 287.7 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 9.1 MB | 3.6 MB | 2.3x | 2.0x |
| rsvelte compiler (napi) | 17.8 MB | 7.5 MB | 4.6x | 4.1x |
| swc (napi) | 10.7 MB | 10.3 MB | 2.8x | 5.6x |
| svelte + acorn-typescript parsers (js bundle) | 500.5 KB | 124.9 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 567.4 KB | 0.7x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 567.5 KB | 0.7x | 0.5x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1891f) | **78.2x** prettier, **79.6x** oxfmt |
| format typescript (5688f) | **34.9x** prettier, **1.85x** oxfmt |
| format css (69f) | **119.5x** prettier, **3.77x** oxfmt |
| parse svelte (1898f) | **2.66x** svelte/compiler, **4.16x** rsvelte-parse |
| parse typescript (5688f) | **1.34x** acorn-typescript, **1.55x** oxc-parser, **0.42x** oxc-parser-raw, **0.53x** yuku-parser, **2.03x** swc |
| parse css (71f) | **0.59x** svelte/compiler, **0.72x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1891f) | **46.7x** prettier, **8.50x** biome-wasm |
| format typescript (5688f) | **20.7x** prettier, **6.94x** biome-wasm, **5.46x** dprint-wasm |
| format css (69f) | **68.5x** prettier, **10.2x** biome-wasm, **6.55x** malva-wasm |
| parse svelte (1898f) | **2.14x** svelte/compiler |
| parse typescript (5688f) | **1.09x** acorn-typescript, **1.41x** oxc-parser-wasm, **0.37x** yuku-parser-wasm |
| parse css (71f) | **0.49x** svelte/compiler, **0.59x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. The canonical parse baselines carry `loc` where tsv’s default rows carry none: acorn-typescript runs with `locations: true` (Svelte’s configuration of acorn), so every node has one; svelte/compiler emits Svelte’s own sparse `loc` (acorn-parsed nodes plus `name_loc`); `parseCss` emits none. So on typescript and svelte the parse cells compare a span-only tree against a loc-bearing one. The read with `loc` on both sides is the `+locations` rows’ cells against the canonical baseline in those two groups’ tables — the span-only parse plus `{locations: true}`’s cost, acorn-exact on typescript, and on svelte a superset of Svelte’s `loc` (every positioned object). oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv/tsv-wasm, so these parse rows are mechanism-matched, and both ASTs are span-only (`start`/`end`, no per-node `loc`); oxc’s writes out default-valued fields tsv omits, so its tree is the larger of the two. oxc-parser-raw is the same native parse as oxc-parser and hands JS the identical tree (byte parity graded per file), but crosses the boundary as a shared buffer that a generated JS deserializer builds the objects from instead of a JSON string — oxc’s undocumented `experimentalRawTransfer`. Deserialization is eager and complete before `parseSync` returns, so the row materializes the full tree like tsv’s default rows, by a transfer mechanism tsv does not ship. Native only, and absent under Bun, where oxc does not support raw transfer. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and like tsv’s own rows), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds) — the mechanism matches tsv’s default rows (serialize, cross, materialize) while the tree it produces is neither tsv’s span-only wire nor the acorn shape `{locations: true}` returns, so the cell is mechanism-matched only. rsvelte-parse returns a compact JSON string the caller parses — the same serialize + boundary + `JSON.parse` mechanism tsv’s default rows measure — but not the same payload: it carries Svelte’s own wire, `loc` on the acorn-parsed nodes plus `name_loc`, where `tsv` carries no `loc` at all and its `+locations` sibling a `loc` on every node, so neither tsv row is payload-matched to it and this cell carries the `loc` on rsvelte’s side. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire either. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter, swc’s CSS package exposes only minify/transform). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× tsv’s span-only JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

**What `{locations: true}` costs over the default** (each binding's `+locations` mean / its default row's mean, higher = more cost; `+locations` = the default span-only parse plus `loc` on every node rebuilt in JS by the shipped `reconstruct_locations`, line table included — exactly what the packages' `{locations: true}` runs): svelte: native 1.41x, wasm 1.34x | typescript: native 1.27x, wasm 1.22x | css: native 1.29x, wasm 1.23x

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
