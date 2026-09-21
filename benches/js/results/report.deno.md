# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-21T18:02:28.380Z — tsv 0.4.1 (939b51f8)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.65       | 16  | 607.35   | 618.30   | 622.53   | 627.19   | 633.97   | 586.06   | 635.67   | baseline                     | baseline |
| tsv-json                    | 4.17       | 21  | 239.40   | 240.05   | 240.19   | 240.26   | 240.79   | 238.80   | 240.92   | 2.53x                        | 2.54x    |
| tsv-wasm-json               | 3.55       | 16  | 282.09   | 282.59   | 283.88   | 286.80   | 288.43   | 281.13   | 288.84   | 2.15x                        | 2.15x    |
| tsv-json-no-locations       | 6.84       | 31  | 146.07   | 146.51   | 146.93   | 147.23   | 149.07   | 145.79   | 149.98   | 4.15x                        | 4.16x    |
| tsv-wasm-json-no-locations  | 5.46       | 27  | 183.10   | 183.66   | 184.02   | 184.34   | 185.85   | 182.66   | 186.39   | 3.31x                        | 3.32x    |
| tsv-internal                | 50.98      | 225 | 19.62    | 19.65    | 19.79    | 19.95    | 20.14    | 19.53    | 20.31    | 30.9x                        | 31.0x    |
| tsv-wasm-internal           | 30.07      | 136 | 33.26    | 33.30    | 33.39    | 33.80    | 33.97    | 33.16    | 34.10    | 18.3x                        | 18.3x    |
| rsvelte-parse               | 1.79       | 9   | 559.16   | 560.63   | 561.83   | —        | —        | 556.03   | 563.79   | 1.08x                        | 1.09x    |
| rsvelte-parse-skip-expr-loc | 2.72       | 13  | 367.27   | 367.86   | 368.67   | 369.46   | 370.19   | 364.97   | 370.38   | 1.65x                        | 1.65x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 10.1 MB/s, tsv-wasm-json 8.6 MB/s, tsv-json-no-locations 16.5 MB/s, tsv-wasm-json-no-locations 13.2 MB/s, tsv-internal 122.9 MB/s, tsv-wasm-internal 72.5 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.2x tsv-internal, tsv-wasm-json 8.5x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.19       | 16 | 5141.34  | 5232.09  | 5259.58  | 5278.57  | 5306.22  | 5042.42  | 5313.13  | baseline              | baseline |
| tsv        | 14.03      | 59 | 71.23    | 71.41    | 72.27    | 72.45    | 72.65    | 71.01    | 72.82    | 72.5x                 | 72.2x    |
| tsv-wasm   | 8.56       | 41 | 116.72   | 117.37   | 117.64   | 117.72   | 120.01   | 116.36   | 120.75   | 44.2x                 | 44.0x    |
| oxfmt      | 0.19       | 8  | 5301.15  | 5353.19  | 5354.47  | —        | —        | 5236.50  | 5357.03  | 0.97x                 | 0.97x    |
| biome-wasm | 1.10       | 6  | 908.57   | 917.19   | 932.14   | —        | —        | 903.25   | 935.47   | 5.69x                 | 5.66x    |

**Files (intersection):** 945

**Throughput:** prettier 0.5 MB/s, tsv 32.7 MB/s, tsv-wasm 19.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript           | 0.31       | 13 | 3.27    | 3.29    | 3.32    | 3.33    | 3.34    | 3.26    | 3.34    | baseline                      | baseline |
| tsv-json                   | 0.53       | 7  | 1.87    | 1.87    | 1.88    | —       | —       | 1.87    | 1.88    | 1.75x                         | 1.75x    |
| tsv-wasm-json              | 0.48       | 8  | 2.10    | 2.11    | 2.11    | —       | —       | 2.10    | 2.12    | 1.55x                         | 1.56x    |
| tsv-json-no-locations      | 1.12       | 8  | 0.89    | 0.90    | 0.90    | —       | —       | 0.89    | 0.90    | 3.66x                         | 3.66x    |
| tsv-wasm-json-no-locations | 0.93       | 7  | 1.08    | 1.08    | 1.08    | —       | —       | 1.07    | 1.08    | 3.04x                         | 3.04x    |
| tsv-internal               | 9.49       | 47 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 31.0x                         | 31.0x    |
| tsv-wasm-internal          | 5.82       | 30 | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 19.0x                         | 19.0x    |
| oxc-parser                 | 0.79       | 6  | 1.26    | 1.26    | 1.26    | —       | —       | 1.25    | 1.27    | 2.60x                         | 2.60x    |
| oxc-parser-wasm            | 0.72       | 6  | 1.38    | 1.39    | 1.39    | —       | —       | 1.38    | 1.39    | 2.37x                         | 2.37x    |
| yuku-parser                | 2.12       | 9  | 0.48    | 0.48    | 0.51    | —       | —       | 0.45    | 0.58    | 6.93x                         | 6.84x    |
| yuku-parser-wasm           | 2.36       | 11 | 0.43    | 0.44    | 0.44    | 0.50    | 0.56    | 0.40    | 0.57    | 7.71x                         | 7.59x    |
| swc                        | 0.57       | 8  | 1.75    | 1.75    | 1.76    | —       | —       | 1.74    | 1.76    | 1.87x                         | 1.87x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 9.9 MB/s, tsv-wasm-json 8.8 MB/s, tsv-json-no-locations 20.6 MB/s, tsv-wasm-json-no-locations 17.2 MB/s, tsv-internal 175.0 MB/s, tsv-wasm-internal 107.3 MB/s, oxc-parser 14.6 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 39.1 MB/s, yuku-parser-wasm 43.5 MB/s, swc 10.6 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.8x tsv-internal, tsv-wasm-json 12.2x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.08       | 16 | 13.22   | 13.29   | 13.33   | 13.35   | 13.36   | 13.08   | 13.37   | baseline              | baseline |
| tsv         | 2.53       | 13 | 0.39    | 0.39    | 0.40    | 0.40    | 0.40    | 0.39    | 0.40    | 33.5x                 | 33.5x    |
| tsv-wasm    | 1.52       | 7  | 0.66    | 0.66    | 0.66    | —       | —       | 0.66    | 0.66    | 20.1x                 | 20.1x    |
| oxfmt       | 1.10       | 8  | 0.91    | 0.91    | 0.91    | —       | —       | 0.90    | 0.91    | 14.6x                 | 14.6x    |
| biome-wasm  | 0.22       | 6  | 4.56    | 4.56    | 4.56    | —       | —       | 4.55    | 4.56    | 2.90x                 | 2.90x    |
| dprint-wasm | 0.27       | 8  | 3.71    | 3.72    | 3.73    | —       | —       | 3.71    | 3.73    | 3.56x                 | 3.56x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.4 MB/s, tsv 46.8 MB/s, tsv-wasm 28.1 MB/s, oxfmt 20.4 MB/s, biome-wasm 4.1 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 75.80      | 378  | 13.17    | 13.37    | 13.58    | 13.69    | 13.96    | 12.74    | 15.49    | baseline                     | baseline |
| tsv-json          | 50.87      | 221  | 19.63    | 19.87    | 20.41    | 21.04    | 21.84    | 19.36    | 32.81    | 0.67x                        | 0.67x    |
| tsv-wasm-json     | 41.44      | 182  | 24.13    | 24.41    | 24.99    | 25.82    | 27.54    | 23.68    | 37.23    | 0.55x                        | 0.55x    |
| tsv-internal      | 276.55     | 1316 | 3.62     | 3.63     | 3.64     | 3.66     | 3.74     | 3.58     | 4.08     | 3.65x                        | 3.64x    |
| tsv-wasm-internal | 152.42     | 677  | 6.56     | 6.57     | 6.60     | 6.63     | 6.78     | 6.54     | 7.00     | 2.01x                        | 2.01x    |
| postcss           | 80.34      | 399  | 12.39    | 12.64    | 12.87    | 13.00    | 13.47    | 12.01    | 14.02    | 1.06x                        | 1.06x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 30.7 MB/s, tsv-json 20.6 MB/s, tsv-wasm-json 16.8 MB/s, tsv-internal 112.0 MB/s, tsv-wasm-internal 61.7 MB/s, postcss 32.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.4x tsv-internal, tsv-wasm-json 3.7x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.77       | 14  | 566.03   | 567.69   | 575.74   | 583.54   | 593.60   | 561.58   | 596.12   | baseline              | baseline |
| tsv        | 161.80     | 714 | 6.18     | 6.20     | 6.28     | 6.33     | 6.65     | 6.13     | 7.05     | 91.5x                 | 91.6x    |
| tsv-wasm   | 89.42      | 412 | 11.18    | 11.21    | 11.26    | 11.34    | 11.59    | 11.11    | 15.51    | 50.6x                 | 50.6x    |
| oxfmt      | 53.99      | 266 | 18.49    | 18.80    | 19.24    | 19.38    | 19.93    | 17.16    | 20.61    | 30.5x                 | 30.6x    |
| biome-wasm | 10.15      | 46  | 98.12    | 99.51    | 100.54   | 101.94   | 116.29   | 97.36    | 117.43   | 5.74x                 | 5.77x    |
| malva-wasm | 19.40      | 83  | 51.53    | 51.73    | 52.16    | 52.26    | 52.64    | 51.40    | 52.77    | 11.0x                 | 11.0x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 58.9 MB/s, tsv-wasm 32.5 MB/s, oxfmt 19.6 MB/s, biome-wasm 3.7 MB/s, malva-wasm 7.1 MB/s

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
| tsv (ffi) | 3.4 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.3x | 3.0x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 675.4 KB | 0.5x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.6x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.7x | 2.4x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.6x | 2.3x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 5.2x | 4.8x |
| swc (napi) | 32.7 MB | 12.2 MB | 9.7x | 7.9x |
| svelte + acorn-typescript parsers (js bundle) | 497.2 KB | 124.0 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 566.1 KB | 0.9x | 0.6x |
| prettier + parsers (js bundle) | 2.2 MB | 566.3 KB | 0.9x | 0.6x |

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **72.5x** prettier, **74.4x** oxfmt |
| format typescript (2642f) | **33.5x** prettier, **2.29x** oxfmt |
| format css (54f) | **91.5x** prettier, **3.00x** oxfmt |
| parse svelte (951f) | **2.53x** svelte/compiler, **2.34x** rsvelte-parse |
| parse typescript (2642f) | **1.75x** acorn-typescript, **0.67x** oxc-parser, **0.25x** yuku-parser, **0.93x** swc |
| parse css (55f) | **0.67x** svelte/compiler, **0.63x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **44.2x** prettier, **7.76x** biome-wasm |
| format typescript (2642f) | **20.1x** prettier, **6.93x** biome-wasm, **5.66x** dprint-wasm |
| format css (54f) | **50.6x** prettier, **8.81x** biome-wasm, **4.61x** malva-wasm |
| parse svelte (951f) | **2.15x** svelte/compiler |
| parse typescript (2642f) | **1.55x** acorn-typescript, **0.66x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (55f) | **0.55x** svelte/compiler, **0.52x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are mechanism-matched; the payload is not: oxc’s default AST is span-only (`start`/`end`, no per-node `loc`, and no option to add one) where `tsv-json` carries the loc-bearing drop-in AST, so the payload-matched read is the `no-locations` line under each parse group. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and its payload-matched read is the same `no-locations` line), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). The drift's sign names the mechanism: negative means the row got FASTER while measured (still warming up — under-warmed), positive means it got slower (degrading — a leak, a heap tipping over, thermal). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/typescript/yuku-parser-wasm | 3.3% | 10.4% | +0.3% | 11/12 |

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
