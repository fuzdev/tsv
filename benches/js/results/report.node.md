# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-21T18:21:14.710Z — tsv 0.4.1 (939b51f8)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.58       | 16  | 628.85   | 634.96   | 645.43   | 646.84   | 648.05   | 623.85   | 648.35   | baseline                     | baseline |
| tsv-json                    | 3.75       | 18  | 266.50   | 267.20   | 267.61   | 268.32   | 271.03   | 265.13   | 271.71   | 2.37x                        | 2.36x    |
| tsv-wasm-json               | 3.52       | 18  | 283.48   | 284.46   | 285.98   | 286.38   | 287.53   | 281.84   | 287.82   | 2.23x                        | 2.22x    |
| tsv-json-no-locations       | 6.23       | 31  | 160.54   | 161.33   | 161.60   | 161.78   | 161.96   | 159.18   | 162.01   | 3.94x                        | 3.92x    |
| tsv-wasm-json-no-locations  | 5.54       | 28  | 180.30   | 181.06   | 181.25   | 181.44   | 181.61   | 178.98   | 181.63   | 3.50x                        | 3.49x    |
| tsv-internal                | 48.84      | 224 | 20.48    | 20.51    | 20.58    | 20.78    | 20.97    | 20.37    | 21.04    | 30.9x                        | 30.7x    |
| tsv-wasm-internal           | 33.73      | 147 | 29.65    | 29.70    | 29.87    | 30.17    | 30.39    | 29.55    | 30.81    | 21.3x                        | 21.2x    |
| rsvelte-parse               | 1.71       | 8   | 586.84   | 586.89   | 589.20   | —        | —        | 584.06   | 595.76   | 1.08x                        | 1.07x    |
| rsvelte-parse-skip-expr-loc | 2.60       | 14  | 383.92   | 385.15   | 385.50   | 385.88   | 386.33   | 382.52   | 386.44   | 1.65x                        | 1.64x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 9.1 MB/s, tsv-wasm-json 8.5 MB/s, tsv-json-no-locations 15.0 MB/s, tsv-wasm-json-no-locations 13.4 MB/s, tsv-internal 117.8 MB/s, tsv-wasm-internal 81.3 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.0x tsv-internal, tsv-wasm-json 9.6x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.18       | 16 | 5.46    | 5.48    | 5.51    | 5.53    | 5.54    | 5.36    | 5.54    | baseline              | baseline |
| tsv        | 13.81      | 59 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 75.3x                 | 75.3x    |
| tsv-wasm   | 9.72       | 44 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 53.0x                 | 53.0x    |
| oxfmt      | 0.18       | 6  | 5.54    | 5.57    | 5.58    | —       | —       | 5.53    | 5.61    | 0.98x                 | 0.98x    |
| biome-wasm | 0.86       | 8  | 1.16    | 1.18    | 1.18    | —       | —       | 1.15    | 1.19    | 4.67x                 | 4.69x    |

**Files (intersection):** 945

**Throughput:** prettier 0.4 MB/s, tsv 32.2 MB/s, tsv-wasm 22.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript           | 0.28       | 16 | 3.53    | 3.54    | 3.55    | 3.55    | 3.56    | 3.52    | 3.56    | baseline                      | baseline |
| tsv-json                   | 0.47       | 7  | 2.12    | 2.12    | 2.12    | —       | —       | 2.12    | 2.12    | 1.67x                         | 1.67x    |
| tsv-wasm-json              | 0.46       | 8  | 2.17    | 2.18    | 2.18    | —       | —       | 2.16    | 2.18    | 1.63x                         | 1.63x    |
| tsv-json-no-locations      | 0.98       | 8  | 1.02    | 1.02    | 1.02    | —       | —       | 1.01    | 1.02    | 3.47x                         | 3.47x    |
| tsv-wasm-json-no-locations | 0.92       | 8  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 3.25x                         | 3.24x    |
| tsv-internal               | 8.71       | 41 | 0.11    | 0.11    | 0.12    | 0.12    | 0.12    | 0.11    | 0.12    | 30.8x                         | 30.8x    |
| tsv-wasm-internal          | 6.80       | 34 | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 24.0x                         | 24.0x    |
| oxc-parser                 | 0.71       | 8  | 1.40    | 1.41    | 1.42    | —       | —       | 1.38    | 1.42    | 2.52x                         | 2.52x    |
| oxc-parser-wasm            | 0.69       | 8  | 1.45    | 1.46    | 1.46    | —       | —       | 1.45    | 1.46    | 2.43x                         | 2.43x    |
| yuku-parser                | 2.32       | 10 | 0.43    | 0.43    | 0.45    | 0.45    | 0.45    | 0.42    | 0.45    | 8.21x                         | 8.20x    |
| yuku-parser-wasm           | 2.72       | 12 | 0.37    | 0.37    | 0.38    | 0.39    | 0.39    | 0.37    | 0.39    | 9.60x                         | 9.59x    |
| swc                        | 0.55       | 8  | 1.82    | 1.82    | 1.82    | —       | —       | 1.82    | 1.82    | 1.94x                         | 1.94x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.2 MB/s, tsv-json 8.7 MB/s, tsv-wasm-json 8.5 MB/s, tsv-json-no-locations 18.1 MB/s, tsv-wasm-json-no-locations 16.9 MB/s, tsv-internal 160.6 MB/s, tsv-wasm-internal 125.3 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.7 MB/s, yuku-parser 42.8 MB/s, yuku-parser-wasm 50.1 MB/s, swc 10.1 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.4x tsv-internal, tsv-wasm-json 14.8x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 14.64   | 14.66   | 14.70   | 14.70   | 14.71   | 14.52   | 14.71   | baseline              | baseline |
| tsv         | 2.43       | 13 | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 35.5x                 | 35.5x    |
| tsv-wasm    | 1.76       | 9  | 0.57    | 0.57    | 0.57    | —       | —       | 0.57    | 0.57    | 25.8x                 | 25.8x    |
| oxfmt       | 1.08       | 8  | 0.92    | 0.93    | 0.93    | —       | —       | 0.91    | 0.93    | 15.8x                 | 15.8x    |
| biome-wasm  | 0.20       | 8  | 4.89    | 4.89    | 4.91    | —       | —       | 4.87    | 4.92    | 2.99x                 | 3.00x    |
| dprint-wasm | 0.30       | 8  | 3.32    | 3.32    | 3.34    | —       | —       | 3.31    | 3.34    | 4.41x                 | 4.41x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.3 MB/s, tsv 44.7 MB/s, tsv-wasm 32.5 MB/s, oxfmt 20.0 MB/s, biome-wasm 3.8 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 80.81      | 371  | 12.27    | 12.69    | 12.88    | 13.07    | 13.41    | 12.04    | 14.17    | baseline                     | baseline |
| tsv-json          | 45.64      | 201  | 21.98    | 22.07    | 24.26    | 24.69    | 25.01    | 21.30    | 26.38    | 0.56x                        | 0.56x    |
| tsv-wasm-json     | 42.82      | 176  | 23.36    | 23.58    | 25.20    | 25.66    | 31.72    | 22.97    | 32.69    | 0.53x                        | 0.53x    |
| tsv-internal      | 254.24     | 1183 | 3.93     | 3.94     | 3.96     | 3.98     | 4.08     | 3.91     | 4.28     | 3.15x                        | 3.12x    |
| tsv-wasm-internal | 171.20     | 777  | 5.84     | 5.86     | 5.89     | 5.93     | 6.09     | 5.81     | 6.17     | 2.12x                        | 2.10x    |
| postcss           | 81.50      | 333  | 12.21    | 12.55    | 12.96    | 13.23    | 13.82    | 12.08    | 14.03    | 1.01x                        | 1.01x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 32.7 MB/s, tsv-json 18.5 MB/s, tsv-wasm-json 17.3 MB/s, tsv-internal 103.0 MB/s, tsv-wasm-internal 69.3 MB/s, postcss 33.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.6x tsv-internal, tsv-wasm-json 4.0x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.71       | 15  | 585.65   | 587.85   | 591.06   | 595.46   | 604.12   | 578.38   | 606.28   | baseline              | baseline |
| tsv        | 148.54     | 696 | 6.73     | 6.75     | 6.80     | 6.85     | 7.18     | 6.67     | 10.53    | 86.8x                 | 87.0x    |
| tsv-wasm   | 103.70     | 484 | 9.64     | 9.67     | 9.71     | 9.78     | 10.15    | 9.57     | 12.90    | 60.6x                 | 60.7x    |
| oxfmt      | 51.21      | 252 | 19.53    | 19.86    | 20.26    | 20.69    | 21.64    | 18.32    | 22.74    | 29.9x                 | 30.0x    |
| biome-wasm | 10.01      | 37  | 100.24   | 103.51   | 114.76   | 115.00   | 117.36   | 96.17    | 118.76   | 5.85x                 | 5.84x    |
| malva-wasm | 21.17      | 91  | 47.25    | 47.32    | 47.73    | 48.01    | 48.59    | 47.11    | 52.08    | 12.4x                 | 12.4x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 54.1 MB/s, tsv-wasm 37.7 MB/s, oxfmt 18.6 MB/s, biome-wasm 3.6 MB/s, malva-wasm 7.7 MB/s

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
| format svelte (945f) | **75.3x** prettier, **76.6x** oxfmt |
| format typescript (2642f) | **35.5x** prettier, **2.24x** oxfmt |
| format css (54f) | **86.8x** prettier, **2.90x** oxfmt |
| parse svelte (951f) | **2.37x** svelte/compiler, **2.20x** rsvelte-parse |
| parse typescript (2642f) | **1.67x** acorn-typescript, **0.66x** oxc-parser, **0.20x** yuku-parser, **0.86x** swc |
| parse css (55f) | **0.56x** svelte/compiler, **0.56x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **53.0x** prettier, **11.3x** biome-wasm |
| format typescript (2642f) | **25.8x** prettier, **8.61x** biome-wasm, **5.85x** dprint-wasm |
| format css (54f) | **60.6x** prettier, **10.4x** biome-wasm, **4.90x** malva-wasm |
| parse svelte (951f) | **2.23x** svelte/compiler |
| parse typescript (2642f) | **1.63x** acorn-typescript, **0.67x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.53x** svelte/compiler, **0.53x** postcss |

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
