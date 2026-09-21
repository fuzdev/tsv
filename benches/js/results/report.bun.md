# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-21T19:03:17.139Z — tsv 0.4.1 (939b51f8)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.24       | 16  | 804.13   | 811.50   | 815.81   | 819.04   | 819.68   | 790.57   | 819.84   | baseline                     | baseline |
| tsv-json                    | 6.13       | 31  | 162.86   | 164.16   | 164.81   | 165.22   | 165.41   | 161.29   | 165.48   | 4.94x                        | 4.94x    |
| tsv-wasm-json               | 5.95       | 27  | 168.20   | 169.06   | 169.95   | 172.52   | 173.81   | 166.87   | 174.10   | 4.79x                        | 4.78x    |
| tsv-json-no-locations       | 8.80       | 42  | 113.69   | 114.14   | 114.84   | 115.30   | 117.12   | 112.06   | 117.34   | 7.09x                        | 7.07x    |
| tsv-wasm-json-no-locations  | 7.96       | 39  | 125.51   | 126.41   | 127.28   | 128.14   | 130.24   | 123.66   | 130.58   | 6.41x                        | 6.41x    |
| tsv-internal                | 54.71      | 241 | 18.27    | 18.33    | 18.48    | 18.61    | 18.86    | 18.17    | 18.95    | 44.1x                        | 44.0x    |
| tsv-wasm-internal           | 35.23      | 161 | 28.38    | 28.43    | 28.54    | 28.77    | 29.17    | 28.28    | 29.30    | 28.4x                        | 28.3x    |
| rsvelte-parse               | 2.06       | 10  | 486.00   | 488.41   | 489.19   | 495.37   | 500.32   | 483.02   | 501.55   | 1.66x                        | 1.65x    |
| rsvelte-parse-skip-expr-loc | 3.05       | 15  | 328.27   | 329.54   | 336.96   | 339.04   | 340.16   | 324.23   | 340.44   | 2.45x                        | 2.45x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.0 MB/s, tsv-json 14.8 MB/s, tsv-wasm-json 14.3 MB/s, tsv-json-no-locations 21.2 MB/s, tsv-wasm-json-no-locations 19.2 MB/s, tsv-internal 132.0 MB/s, tsv-wasm-internal 85.0 MB/s, rsvelte-parse 5.0 MB/s, rsvelte-parse-skip-expr-loc 7.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.9x tsv-internal, tsv-wasm-json 5.9x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.24       | 15 | 4.14    | 4.20    | 4.34    | 4.44    | 4.54    | 4.07    | 4.57    | baseline              | baseline |
| tsv        | 13.58      | 59 | 0.07    | 0.07    | 0.07    | 0.07    | 0.08    | 0.07    | 0.08    | 56.4x                 | 56.2x    |
| tsv-wasm   | 9.49       | 45 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 39.4x                 | 39.3x    |
| oxfmt      | 0.22       | 8  | 4.61    | 4.63    | 4.65    | —       | —       | 4.49    | 4.71    | 0.91x                 | 0.90x    |
| biome-wasm | 0.94       | 8  | 1.07    | 1.08    | 1.08    | —       | —       | 1.03    | 1.08    | 3.91x                 | 3.87x    |

**Files (intersection):** 945

**Throughput:** prettier 0.6 MB/s, tsv 31.6 MB/s, tsv-wasm 22.1 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.2 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- | -------- |
| acorn-typescript           | 0.18       | 16 | 5557.76  | 5571.99  | 5598.05  | 5606.13  | 5623.50  | 5516.15  | 5627.84  | baseline                      | baseline |
| tsv-json                   | 0.87       | 8  | 1144.49  | 1146.31  | 1147.44  | —        | —        | 1139.99  | 1147.92  | 4.86x                         | 4.86x    |
| tsv-wasm-json              | 0.89       | 8  | 1123.72  | 1129.68  | 1130.25  | —        | —        | 1118.98  | 1130.67  | 4.94x                         | 4.95x    |
| tsv-json-no-locations      | 1.56       | 8  | 641.99   | 647.74   | 649.07   | —        | —        | 635.33   | 650.08   | 8.65x                         | 8.66x    |
| tsv-wasm-json-no-locations | 1.48       | 8  | 674.89   | 678.32   | 680.98   | —        | —        | 665.66   | 681.84   | 8.24x                         | 8.24x    |
| tsv-internal               | 10.61      | 51 | 94.19    | 94.41    | 94.83    | 95.04    | 96.99    | 93.65    | 98.98    | 59.0x                         | 59.0x    |
| tsv-wasm-internal          | 7.22       | 35 | 138.41   | 139.03   | 139.16   | 139.31   | 139.65   | 137.97   | 139.70   | 40.1x                         | 40.2x    |
| oxc-parser                 | 1.14       | 8  | 874.48   | 877.00   | 879.91   | —        | —        | 867.07   | 881.73   | 6.36x                         | 6.36x    |
| oxc-parser-wasm            | 0.85       | 8  | 1158.42  | 1191.22  | 1213.23  | —        | —        | 1131.64  | 1251.69  | 4.75x                         | 4.80x    |
| yuku-parser                | 2.87       | 13 | 344.23   | 366.59   | 383.87   | 395.97   | 402.49   | 338.50   | 404.12   | 16.0x                         | 16.1x    |
| yuku-parser-wasm           | 3.51       | 17 | 283.19   | 292.98   | 301.67   | 307.24   | 327.99   | 276.14   | 333.17   | 19.5x                         | 19.6x    |
| swc                        | 0.73       | 8  | 1370.22  | 1371.19  | 1373.78  | —        | —        | 1363.36  | 1374.44  | 4.06x                         | 4.06x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.3 MB/s, tsv-json 16.1 MB/s, tsv-wasm-json 16.4 MB/s, tsv-json-no-locations 28.7 MB/s, tsv-wasm-json-no-locations 27.3 MB/s, tsv-internal 195.7 MB/s, tsv-wasm-internal 133.1 MB/s, oxc-parser 21.1 MB/s, oxc-parser-wasm 15.7 MB/s, yuku-parser 52.9 MB/s, yuku-parser-wasm 64.7 MB/s, swc 13.5 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.1x tsv-internal, tsv-wasm-json 8.1x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 14.07   | 14.15   | 14.18   | 14.21   | 14.26   | 13.79   | 14.27   | baseline              | baseline |
| tsv         | 2.45       | 13 | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 34.4x                 | 34.5x    |
| tsv-wasm    | 1.72       | 9  | 0.58    | 0.58    | 0.58    | —       | —       | 0.58    | 0.58    | 24.2x                 | 24.3x    |
| oxfmt       | 1.08       | 7  | 0.93    | 0.93    | 0.94    | —       | —       | 0.92    | 0.97    | 15.2x                 | 15.1x    |
| biome-wasm  | 0.23       | 8  | 4.39    | 4.41    | 4.42    | —       | —       | 4.33    | 4.42    | 3.20x                 | 3.20x    |
| dprint-wasm | 0.31       | 8  | 3.24    | 3.24    | 3.24    | —       | —       | 3.23    | 3.24    | 4.34x                 | 4.35x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.3 MB/s, tsv 45.2 MB/s, tsv-wasm 31.8 MB/s, oxfmt 19.9 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 48.01      | 236  | 20.88    | 21.15    | 21.40    | 21.55    | 23.18    | 19.85    | 23.74    | baseline                     | baseline |
| tsv-json          | 63.53      | 290  | 15.64    | 16.13    | 16.52    | 16.75    | 17.68    | 15.30    | 19.02    | 1.32x                        | 1.33x    |
| tsv-wasm-json     | 66.08      | 283  | 15.14    | 15.21    | 15.46    | 15.87    | 16.36    | 14.96    | 19.13    | 1.38x                        | 1.38x    |
| tsv-internal      | 288.47     | 1279 | 3.47     | 3.48     | 3.51     | 3.53     | 3.60     | 3.44     | 3.79     | 6.01x                        | 6.02x    |
| tsv-wasm-internal | 192.47     | 858  | 5.20     | 5.20     | 5.24     | 5.27     | 5.39     | 5.17     | 5.54     | 4.01x                        | 4.02x    |
| postcss           | 72.10      | 305  | 13.86    | 14.11    | 14.85    | 14.96    | 17.73    | 13.39    | 21.92    | 1.50x                        | 1.51x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.4 MB/s, tsv-json 25.7 MB/s, tsv-wasm-json 26.8 MB/s, tsv-internal 116.8 MB/s, tsv-wasm-internal 78.0 MB/s, postcss 29.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.5x tsv-internal, tsv-wasm-json 2.9x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 2.40       | 16  | 416.21   | 420.05   | 425.14   | 425.39   | 425.41   | 410.36   | 425.41   | baseline              | baseline |
| tsv        | 154.19     | 713 | 6.48     | 6.51     | 6.57     | 6.61     | 6.82     | 6.42     | 8.54     | 64.3x                 | 64.2x    |
| tsv-wasm   | 109.73     | 474 | 9.11     | 9.14     | 9.22     | 9.27     | 9.52     | 9.05     | 9.66     | 45.8x                 | 45.7x    |
| oxfmt      | 53.51      | 260 | 18.66    | 19.04    | 19.40    | 19.78    | 21.80    | 17.28    | 21.95    | 22.3x                 | 22.3x    |
| biome-wasm | 12.01      | 48  | 83.03    | 84.03    | 85.30    | 100.03   | 116.19   | 81.65    | 118.17   | 5.01x                 | 5.01x    |
| malva-wasm | 17.94      | 80  | 55.75    | 55.90    | 56.31    | 56.61    | 57.01    | 55.46    | 57.09    | 7.48x                 | 7.47x    |

**Files (intersection):** 54

**Throughput:** prettier 0.9 MB/s, tsv 56.1 MB/s, tsv-wasm 39.9 MB/s, oxfmt 19.5 MB/s, biome-wasm 4.4 MB/s, malva-wasm 6.5 MB/s

**Coverage:** prettier 55/55 (100%), tsv 55/55 (100%), tsv-wasm 55/55 (100%), oxfmt 55/55 (100%), biome-wasm 54/55 (98%), malva-wasm 55/55 (100%)

**Omitted from every row's timed set:** 1 of 55 files, 10.2% of the group's bytes (1.8% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.3 MB | 867.2 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 968.9 KB | 377.0 KB | 0.4x | 0.4x |
| tsv-wasm | 2.5 MB | 966.4 KB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 17.6x | 11.8x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.2x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.4x |
| tsv (ffi) | 3.4 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 675.4 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.0x | 2.7x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.4x | 2.1x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.7x | 4.3x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.7x | 7.2x |
| svelte + acorn-typescript parsers (js bundle) | 497.2 KB | 124.0 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 566.1 KB | 0.9x | 0.6x |
| prettier + parsers (js bundle) | 2.2 MB | 566.3 KB | 0.9x | 0.6x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **56.4x** prettier, **62.3x** oxfmt |
| format typescript (2642f) | **34.4x** prettier, **2.27x** oxfmt |
| format css (54f) | **64.3x** prettier, **2.88x** oxfmt |
| parse svelte (951f) | **4.94x** svelte/compiler, **2.98x** rsvelte-parse |
| parse typescript (2642f) | **4.86x** acorn-typescript, **0.76x** oxc-parser, **0.30x** yuku-parser, **1.20x** swc |
| parse css (55f) | **1.32x** svelte/compiler, **0.88x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **39.4x** prettier, **10.1x** biome-wasm |
| format typescript (2642f) | **24.2x** prettier, **7.57x** biome-wasm, **5.58x** dprint-wasm |
| format css (54f) | **45.8x** prettier, **9.13x** biome-wasm, **6.12x** malva-wasm |
| parse svelte (951f) | **4.79x** svelte/compiler |
| parse typescript (2642f) | **4.94x** acorn-typescript, **1.04x** oxc-parser-wasm, **0.25x** yuku-parser-wasm |
| parse css (55f) | **1.38x** svelte/compiler, **0.92x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are mechanism-matched; the payload is not: oxc’s default AST is span-only (`start`/`end`, no per-node `loc`, and no option to add one) where `tsv-json` carries the loc-bearing drop-in AST, so the payload-matched read is the `no-locations` line under each parse group. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and its payload-matched read is the same `no-locations` line), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

11 files skipped, 22 unique file+error combinations — Svelte 6, TypeScript 4, CSS 1 files.

**Per-benchmark skip counts:**
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
