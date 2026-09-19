# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-19T22:27:22.227Z — tsv 0.4.1 (8837350f)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.64       | 16  | 603.58   | 620.13   | 623.81   | 625.86   | 630.01   | 596.26   | 631.05   | baseline                     | baseline |
| tsv-json                    | 4.15       | 21  | 241.00   | 241.33   | 241.86   | 241.92   | 242.03   | 239.94   | 242.06   | 2.53x                        | 2.50x    |
| tsv-wasm-json               | 3.53       | 17  | 283.20   | 284.04   | 284.71   | 286.58   | 290.58   | 281.67   | 291.58   | 2.15x                        | 2.13x    |
| tsv-json-no-locations       | 6.78       | 28  | 147.42   | 147.92   | 148.29   | 148.48   | 150.38   | 147.19   | 151.28   | 4.14x                        | 4.09x    |
| tsv-wasm-json-no-locations  | 5.42       | 27  | 184.37   | 185.08   | 185.34   | 185.43   | 187.59   | 183.80   | 188.38   | 3.30x                        | 3.27x    |
| tsv-internal                | 50.54      | 228 | 19.79    | 19.82    | 19.91    | 20.10    | 20.35    | 19.70    | 20.39    | 30.8x                        | 30.5x    |
| tsv-wasm-internal           | 29.56      | 131 | 33.82    | 33.86    | 34.07    | 34.32    | 34.55    | 33.74    | 38.27    | 18.0x                        | 17.8x    |
| rsvelte-parse               | 1.78       | 8   | 561.47   | 561.81   | 562.92   | —        | —        | 560.54   | 564.48   | 1.09x                        | 1.08x    |
| rsvelte-parse-skip-expr-loc | 2.72       | 14  | 367.88   | 368.25   | 369.32   | 370.03   | 370.76   | 364.32   | 370.94   | 1.66x                        | 1.64x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 10.0 MB/s, tsv-wasm-json 8.5 MB/s, tsv-json-no-locations 16.4 MB/s, tsv-wasm-json-no-locations 13.1 MB/s, tsv-internal 121.9 MB/s, tsv-wasm-internal 71.3 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.2x tsv-internal, tsv-wasm-json 8.4x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 0.19       | 16 | 5234.30  | 5271.09  | 5342.67  | 5384.67  | 5392.22  | 5135.15  | 5394.11  | baseline              | baseline |
| tsv        | 13.96      | 57 | 71.64    | 71.99    | 72.40    | 72.43    | 72.65    | 71.42    | 72.88    | 73.1x                 | 73.1x    |
| tsv-wasm   | 8.41       | 41 | 118.74   | 119.40   | 119.59   | 119.81   | 121.51   | 118.31   | 122.30   | 44.1x                 | 44.1x    |
| oxfmt      | 0.19       | 8  | 5308.54  | 5323.01  | 5336.39  | —        | —        | 5271.37  | 5352.31  | 0.99x                 | 0.99x    |
| biome-wasm | 1.09       | 7  | 916.19   | 928.10   | 959.90   | —        | —        | 905.02   | 968.99   | 5.71x                 | 5.71x    |

**Files (intersection):** 945

**Throughput:** prettier 0.4 MB/s, tsv 32.5 MB/s, tsv-wasm 19.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.5 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript           | 0.30       | 14 | 3.31    | 3.32    | 3.35    | 3.37    | 3.37    | 3.27    | 3.37    | baseline                      | baseline |
| tsv-json                   | 0.53       | 7  | 1.88    | 1.89    | 1.90    | —       | —       | 1.88    | 1.90    | 1.75x                         | 1.76x    |
| tsv-wasm-json              | 0.47       | 7  | 2.11    | 2.11    | 2.12    | —       | —       | 2.10    | 2.14    | 1.57x                         | 1.57x    |
| tsv-json-no-locations      | 1.11       | 8  | 0.90    | 0.90    | 0.90    | —       | —       | 0.90    | 0.90    | 3.67x                         | 3.68x    |
| tsv-wasm-json-no-locations | 0.92       | 8  | 1.09    | 1.09    | 1.09    | —       | —       | 1.08    | 1.09    | 3.05x                         | 3.05x    |
| tsv-internal               | 9.59       | 48 | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 0.10    | 31.7x                         | 31.8x    |
| tsv-wasm-internal          | 5.82       | 30 | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 0.17    | 19.3x                         | 19.3x    |
| oxc-parser                 | 0.79       | 6  | 1.27    | 1.27    | 1.28    | —       | —       | 1.26    | 1.28    | 2.60x                         | 2.61x    |
| oxc-parser-wasm            | 0.72       | 7  | 1.39    | 1.39    | 1.40    | —       | —       | 1.39    | 1.41    | 2.38x                         | 2.39x    |
| yuku-parser                | 2.14       | 10 | 0.47    | 0.47    | 0.48    | 0.48    | 0.48    | 0.45    | 0.48    | 7.08x                         | 7.04x    |
| yuku-parser-wasm           | 2.32       | 10 | 0.44    | 0.44    | 0.48    | 0.51    | 0.54    | 0.42    | 0.55    | 7.66x                         | 7.56x    |
| swc                        | 0.58       | 7  | 1.72    | 1.73    | 1.74    | —       | —       | 1.71    | 1.75    | 1.92x                         | 1.92x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 9.8 MB/s, tsv-wasm-json 8.7 MB/s, tsv-json-no-locations 20.5 MB/s, tsv-wasm-json-no-locations 17.0 MB/s, tsv-internal 176.9 MB/s, tsv-wasm-internal 107.4 MB/s, oxc-parser 14.5 MB/s, oxc-parser-wasm 13.3 MB/s, yuku-parser 39.5 MB/s, yuku-parser-wasm 42.7 MB/s, swc 10.7 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.1x tsv-internal, tsv-wasm-json 12.3x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.08       | 16 | 13.21   | 13.25   | 13.28   | 13.29   | 13.32   | 13.11   | 13.33   | baseline              | baseline |
| tsv         | 2.51       | 13 | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 0.40    | 33.2x                 | 33.2x    |
| tsv-wasm    | 1.49       | 6  | 0.67    | 0.67    | 0.67    | —       | —       | 0.67    | 0.68    | 19.7x                 | 19.7x    |
| oxfmt       | 1.08       | 8  | 0.93    | 0.94    | 0.94    | —       | —       | 0.91    | 0.95    | 14.2x                 | 14.2x    |
| biome-wasm  | 0.22       | 8  | 4.60    | 4.61    | 4.61    | —       | —       | 4.57    | 4.62    | 2.87x                 | 2.87x    |
| dprint-wasm | 0.26       | 7  | 3.81    | 3.81    | 3.82    | —       | —       | 3.81    | 3.82    | 3.47x                 | 3.47x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.4 MB/s, tsv 46.4 MB/s, tsv-wasm 27.5 MB/s, oxfmt 19.9 MB/s, biome-wasm 4.0 MB/s, dprint-wasm 4.8 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 75.88      | 379  | 13.14    | 13.36    | 13.47    | 13.59    | 13.76    | 12.87    | 14.10    | baseline                     | baseline |
| tsv-json          | 51.06      | 233  | 19.62    | 19.75    | 20.34    | 21.20    | 21.93    | 19.19    | 31.88    | 0.67x                        | 0.67x    |
| tsv-wasm-json     | 41.63      | 182  | 23.98    | 24.26    | 25.18    | 26.14    | 26.92    | 23.67    | 35.68    | 0.55x                        | 0.55x    |
| tsv-internal      | 276.45     | 1309 | 3.62     | 3.62     | 3.64     | 3.66     | 3.76     | 3.59     | 3.92     | 3.64x                        | 3.63x    |
| tsv-wasm-internal | 152.03     | 733  | 6.57     | 6.60     | 6.62     | 6.65     | 6.80     | 6.53     | 6.87     | 2.00x                        | 2.00x    |
| postcss           | 80.66      | 402  | 12.37    | 12.61    | 12.80    | 13.03    | 13.29    | 11.94    | 16.80    | 1.06x                        | 1.06x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 30.7 MB/s, tsv-json 20.7 MB/s, tsv-wasm-json 16.9 MB/s, tsv-internal 112.0 MB/s, tsv-wasm-internal 61.6 MB/s, postcss 32.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.4x tsv-internal, tsv-wasm-json 3.7x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.81       | 14  | 552.36   | 556.58   | 570.34   | 577.06   | 578.75   | 546.51   | 579.18   | baseline              | baseline |
| tsv        | 161.12     | 683 | 6.21     | 6.23     | 6.33     | 6.39     | 6.62     | 6.16     | 6.87     | 89.0x                 | 89.0x    |
| tsv-wasm   | 89.63      | 404 | 11.16    | 11.18    | 11.24    | 11.35    | 11.58    | 11.10    | 11.96    | 49.5x                 | 49.5x    |
| oxfmt      | 53.97      | 265 | 18.54    | 18.83    | 19.15    | 19.41    | 20.12    | 17.06    | 20.41    | 29.8x                 | 29.8x    |
| biome-wasm | 10.16      | 42  | 98.27    | 99.56    | 100.36   | 100.71   | 117.43   | 97.48    | 117.72   | 5.61x                 | 5.62x    |
| malva-wasm | 19.41      | 90  | 51.53    | 51.62    | 52.00    | 52.24    | 52.69    | 51.27    | 52.95    | 10.7x                 | 10.7x    |

**Files (intersection):** 54

**Throughput:** prettier 0.7 MB/s, tsv 58.6 MB/s, tsv-wasm 32.6 MB/s, oxfmt 19.6 MB/s, biome-wasm 3.7 MB/s, malva-wasm 7.1 MB/s

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

_`vs tsv` divides native rows by `tsv (ffi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **73.1x** prettier, **74.1x** oxfmt |
| format typescript (2642f) | **33.2x** prettier, **2.33x** oxfmt |
| format css (54f) | **89.0x** prettier, **2.99x** oxfmt |
| parse svelte (951f) | **2.53x** svelte/compiler, **2.33x** rsvelte-parse |
| parse typescript (2642f) | **1.75x** acorn-typescript, **0.67x** oxc-parser, **0.25x** yuku-parser, **0.91x** swc |
| parse css (55f) | **0.67x** svelte/compiler, **0.63x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **44.1x** prettier, **7.72x** biome-wasm |
| format typescript (2642f) | **19.7x** prettier, **6.86x** biome-wasm, **5.69x** dprint-wasm |
| format css (54f) | **49.5x** prettier, **8.82x** biome-wasm, **4.62x** malva-wasm |
| parse svelte (951f) | **2.15x** svelte/compiler |
| parse typescript (2642f) | **1.57x** acorn-typescript, **0.66x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (55f) | **0.55x** svelte/compiler, **0.52x** postcss |

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
