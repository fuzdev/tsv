# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-10-02T17:05:50.124Z — tsv 0.5.0 (120567d4)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@446268fab](https://github.com/fuzdev/corpora/tree/446268faba0afa32eeed2d35ae53f6b8d3ae4701) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_repos/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.57       | 16  | 637.38   | 641.90   | 650.90   | 651.62   | 651.94   | 623.84   | 652.01   | baseline                     | baseline |
| tsv-json                    | 3.88       | 20  | 257.53   | 258.46   | 259.71   | 259.99   | 260.42   | 256.14   | 260.53   | 2.48x                        | 2.47x    |
| tsv-wasm-json               | 3.66       | 19  | 273.11   | 273.74   | 274.53   | 275.19   | 275.56   | 271.04   | 275.65   | 2.34x                        | 2.33x    |
| tsv-json-no-locations       | 6.50       | 32  | 153.63   | 154.05   | 154.44   | 154.59   | 155.15   | 152.66   | 155.33   | 4.16x                        | 4.15x    |
| tsv-wasm-json-no-locations  | 5.86       | 30  | 170.63   | 171.25   | 171.46   | 171.59   | 171.84   | 169.14   | 171.90   | 3.74x                        | 3.74x    |
| tsv-internal                | 59.57      | 266 | 16.79    | 16.82    | 16.93    | 17.03    | 17.27    | 16.72    | 17.58    | 38.1x                        | 38.0x    |
| tsv-wasm-internal           | 40.39      | 178 | 24.76    | 24.80    | 24.97    | 25.15    | 25.35    | 24.64    | 25.42    | 25.8x                        | 25.7x    |
| rsvelte-parse               | 1.71       | 9   | 584.62   | 585.72   | 587.67   | —        | —        | 582.83   | 588.67   | 1.09x                        | 1.09x    |
| rsvelte-parse-skip-expr-loc | 2.61       | 14  | 382.51   | 383.16   | 384.10   | 384.39   | 384.42   | 380.74   | 384.43   | 1.67x                        | 1.67x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 9.4 MB/s, tsv-wasm-json 8.8 MB/s, tsv-json-no-locations 15.7 MB/s, tsv-wasm-json-no-locations 14.1 MB/s, tsv-internal 143.7 MB/s, tsv-wasm-internal 97.4 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 15.4x tsv-internal, tsv-wasm-json 11.0x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.19       | 16 | 5.40    | 5.42    | 5.46    | 5.48    | 5.48    | 5.35    | 5.48    | baseline              | baseline |
| tsv        | 14.79      | 62 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 79.9x                 | 79.8x    |
| tsv-wasm   | 10.36      | 42 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 56.0x                 | 55.9x    |
| oxfmt      | 0.18       | 8  | 5.49    | 5.51    | 5.51    | —       | —       | 5.42    | 5.51    | 0.99x                 | 0.98x    |
| biome-wasm | 0.85       | 8  | 1.17    | 1.20    | 1.20    | —       | —       | 1.15    | 1.20    | 4.60x                 | 4.61x    |

**Files (intersection):** 945

**Throughput:** prettier 0.4 MB/s, tsv 34.4 MB/s, tsv-wasm 24.1 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript           | 0.29       | 16 | 3.47    | 3.50    | 3.50    | 3.50    | 3.51    | 3.45    | 3.51    | baseline                      | baseline |
| tsv-json                   | 0.48       | 8  | 2.07    | 2.08    | 2.08    | —       | —       | 2.07    | 2.08    | 1.68x                         | 1.67x    |
| tsv-wasm-json              | 0.47       | 8  | 2.12    | 2.12    | 2.13    | —       | —       | 2.11    | 2.13    | 1.64x                         | 1.64x    |
| tsv-json-no-locations      | 1.01       | 6  | 0.99    | 0.99    | 0.99    | —       | —       | 0.99    | 0.99    | 3.51x                         | 3.50x    |
| tsv-wasm-json-no-locations | 0.95       | 8  | 1.05    | 1.05    | 1.05    | —       | —       | 1.05    | 1.05    | 3.31x                         | 3.31x    |
| tsv-internal               | 9.79       | 48 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 34.0x                         | 34.0x    |
| tsv-wasm-internal          | 7.28       | 37 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 25.3x                         | 25.3x    |
| oxc-parser                 | 0.71       | 7  | 1.40    | 1.40    | 1.41    | —       | —       | 1.40    | 1.41    | 2.48x                         | 2.48x    |
| oxc-parser-wasm            | 0.69       | 8  | 1.44    | 1.44    | 1.45    | —       | —       | 1.43    | 1.45    | 2.41x                         | 2.41x    |
| yuku-parser                | 2.36       | 10 | 0.42    | 0.43    | 0.44    | 0.44    | 0.44    | 0.42    | 0.44    | 8.20x                         | 8.20x    |
| yuku-parser-wasm           | 2.71       | 12 | 0.37    | 0.37    | 0.39    | 0.39    | 0.39    | 0.37    | 0.39    | 9.41x                         | 9.38x    |
| swc                        | 0.55       | 7  | 1.81    | 1.81    | 1.81    | —       | —       | 1.81    | 1.82    | 1.92x                         | 1.91x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.9 MB/s, tsv-wasm-json 8.7 MB/s, tsv-json-no-locations 18.6 MB/s, tsv-wasm-json-no-locations 17.6 MB/s, tsv-internal 180.4 MB/s, tsv-wasm-internal 134.2 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.5 MB/s, yuku-parser-wasm 49.9 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 20.3x tsv-internal, tsv-wasm-json 15.4x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 16 | 15.35   | 15.39   | 15.41   | 15.44   | 15.49   | 15.27   | 15.50   | baseline              | baseline |
| tsv         | 2.56       | 13 | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 0.39    | 39.3x                 | 39.2x    |
| tsv-wasm    | 1.83       | 10 | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 0.55    | 28.0x                 | 28.0x    |
| oxfmt       | 1.08       | 8  | 0.93    | 0.93    | 0.93    | —       | —       | 0.91    | 0.93    | 16.6x                 | 16.6x    |
| biome-wasm  | 0.20       | 8  | 4.94    | 4.95    | 4.96    | —       | —       | 4.92    | 4.97    | 3.11x                 | 3.11x    |
| dprint-wasm | 0.30       | 8  | 3.29    | 3.30    | 3.30    | —       | —       | 3.29    | 3.30    | 4.66x                 | 4.66x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.2 MB/s, tsv 47.2 MB/s, tsv-wasm 33.7 MB/s, oxfmt 20.0 MB/s, biome-wasm 3.7 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 82.29      | 402  | 12.07    | 12.32    | 12.55    | 12.63    | 13.05    | 11.88    | 13.42    | baseline                     | baseline |
| tsv-json          | 48.67      | 185  | 20.54    | 20.63    | 22.95    | 23.45    | 23.91    | 20.06    | 25.84    | 0.59x                        | 0.59x    |
| tsv-wasm-json     | 47.14      | 190  | 21.20    | 21.43    | 23.13    | 23.49    | 30.14    | 21.01    | 31.10    | 0.57x                        | 0.57x    |
| tsv-internal      | 364.54     | 1665 | 2.74     | 2.75     | 2.76     | 2.79     | 2.87     | 2.72     | 2.98     | 4.43x                        | 4.40x    |
| tsv-wasm-internal | 242.20     | 1097 | 4.13     | 4.14     | 4.16     | 4.18     | 4.28     | 4.09     | 4.74     | 2.94x                        | 2.92x    |
| postcss           | 83.32      | 321  | 11.95    | 12.29    | 12.44    | 12.87    | 13.06    | 11.85    | 13.45    | 1.01x                        | 1.01x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 32.3 MB/s, tsv-json 19.1 MB/s, tsv-wasm-json 18.5 MB/s, tsv-internal 143.2 MB/s, tsv-wasm-internal 95.2 MB/s, postcss 32.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.5x tsv-internal, tsv-wasm-json 5.1x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.72       | 16  | 581.35   | 584.44   | 587.38   | 591.56   | 599.19   | 572.94   | 601.10   | baseline              | baseline |
| tsv        | 200.55     | 923 | 4.99     | 5.00     | 5.03     | 5.07     | 5.27     | 4.94     | 8.07     | 116.6x                | 116.6x   |
| tsv-wasm   | 139.09     | 636 | 7.19     | 7.21     | 7.25     | 7.29     | 7.47     | 7.15     | 10.27    | 80.9x                 | 80.9x    |
| oxfmt      | 52.09      | 254 | 19.10    | 19.55    | 20.06    | 20.37    | 22.36    | 17.91    | 23.99    | 30.3x                 | 30.4x    |
| biome-wasm | 9.96       | 39  | 99.31    | 109.38   | 112.50   | 114.27   | 116.99   | 95.04    | 117.97   | 5.80x                 | 5.85x    |
| malva-wasm | 21.16      | 95  | 47.25    | 47.36    | 47.75    | 47.98    | 48.23    | 47.03    | 51.11    | 12.3x                 | 12.3x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 71.0 MB/s, tsv-wasm 49.2 MB/s, oxfmt 18.4 MB/s, biome-wasm 3.5 MB/s, malva-wasm 7.5 MB/s

**Coverage:** prettier 55/55 (100%), tsv 55/55 (100%), tsv-wasm 55/55 (100%), oxfmt 55/55 (100%), biome-wasm 54/55 (98%), malva-wasm 55/55 (100%)

**Omitted from every row's timed set:** 1 of 55 files, 9.9% of the group's bytes (1.8% of its files) — by row: biome-wasm 1 (1 harvest_artifact). Each is a reviewed entry in `lib/perf_omit.ts`.

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv-format-wasm | 2.4 MB | 937.0 KB | 0.9x | 0.9x |
| tsv-parse-wasm | 1.0 MB | 398.9 KB | 0.4x | 0.4x |
| tsv-wasm | 2.7 MB | 1.0 MB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 16.5x | 10.9x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.6 MB | 1.7 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 702.2 KB | 0.4x | 0.4x |
| tsv (napi) | 4.0 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.8x | 2.5x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.5x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.3x | 2.0x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.3x | 1.9x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.5x | 4.0x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.3x | 6.7x |
| svelte + acorn-typescript parsers (js bundle) | 497.2 KB | 124.0 KB | 0.2x | 0.1x |
| prettier + svelte plugin (js bundle) | 2.2 MB | 566.1 KB | 0.8x | 0.5x |
| prettier + parsers (js bundle) | 2.2 MB | 566.3 KB | 0.8x | 0.5x |

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm and js-bundle rows by `tsv-wasm`, the portable artifact a JS bundle stands beside. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes. The `js bundle` rows are SYNTHESIZED, not shipped: the canonical tools publish no single artifact, so each is a minified, tree-shaken bundle of the minimum one capability needs (`benches/js/size_bundles/`), built by `deno bundle` during this run._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **79.9x** prettier, **81.1x** oxfmt |
| format typescript (2642f) | **39.3x** prettier, **2.36x** oxfmt |
| format css (54f) | **116.6x** prettier, **3.85x** oxfmt |
| parse svelte (951f) | **2.48x** svelte/compiler, **2.27x** rsvelte-parse |
| parse typescript (2642f) | **1.68x** acorn-typescript, **0.68x** oxc-parser, **0.20x** yuku-parser, **0.87x** swc |
| parse css (55f) | **0.59x** svelte/compiler, **0.58x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **56.0x** prettier, **12.2x** biome-wasm |
| format typescript (2642f) | **28.0x** prettier, **9.02x** biome-wasm, **6.01x** dprint-wasm |
| format css (54f) | **80.9x** prettier, **14.0x** biome-wasm, **6.57x** malva-wasm |
| parse svelte (951f) | **2.34x** svelte/compiler |
| parse typescript (2642f) | **1.64x** acorn-typescript, **0.68x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
| parse css (55f) | **0.57x** svelte/compiler, **0.57x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS and CSS natively; only its svelte row routes through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript and css, and the svelte ratio is a prettier-pipeline number in oxfmt packaging. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv-wasm-json, so these parse rows are mechanism-matched; the payload is not: oxc’s default AST is span-only (`start`/`end`, no per-node `loc`, and no option to add one) where `tsv-json` carries the loc-bearing drop-in AST, so the payload-matched read is the `no-locations` line under each parse group. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser, so it is span-only like oxc and its payload-matched read is the same `no-locations` line), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since none of the Rust CSS tools considered exposes a parse call to JS (Lightning CSS hands a tree to a visitor only mid-transform, Biome surfaces no parser, malva is a formatter). Not payload-matched either: it keeps selectors as strings where `parseCss` (and so tsv) parses them into selector nodes — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus, though the `source` positions and `raws` on every node leave it about as many JS objects as `parseCss` builds. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv-wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

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
