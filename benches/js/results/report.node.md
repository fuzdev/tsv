# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-19T22:46:13.085Z — tsv 0.4.1 (8837350f)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@b4c8d2862](https://github.com/fuzdev/corpora/tree/b4c8d2862dc13b0e04512dc2c5dd5f79a6c74c1b) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.57.0, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler             | 1.57       | 14  | 634.10   | 641.61   | 649.51   | 651.44   | 655.54   | 630.58   | 656.56   | baseline                     | baseline |
| tsv-json                    | 3.71       | 19  | 269.17   | 270.12   | 270.64   | 270.99   | 272.68   | 267.97   | 273.10   | 2.36x                        | 2.36x    |
| tsv-wasm-json               | 3.49       | 18  | 285.92   | 287.33   | 287.78   | 287.98   | 288.80   | 284.83   | 289.00   | 2.22x                        | 2.22x    |
| tsv-json-no-locations       | 6.19       | 25  | 161.29   | 161.92   | 162.30   | 162.65   | 163.56   | 160.55   | 163.94   | 3.94x                        | 3.93x    |
| tsv-wasm-json-no-locations  | 5.55       | 27  | 180.17   | 180.66   | 181.21   | 181.83   | 182.65   | 178.54   | 182.87   | 3.53x                        | 3.52x    |
| tsv-internal                | 49.28      | 213 | 20.30    | 20.33    | 20.48    | 20.68    | 21.00    | 20.16    | 24.38    | 31.3x                        | 31.2x    |
| tsv-wasm-internal           | 33.93      | 160 | 29.47    | 29.53    | 29.72    | 29.93    | 30.69    | 29.31    | 30.72    | 21.6x                        | 21.5x    |
| rsvelte-parse               | 1.70       | 7   | 589.71   | 592.94   | 593.58   | —        | —        | 588.30   | 594.05   | 1.08x                        | 1.08x    |
| rsvelte-parse-skip-expr-loc | 2.61       | 14  | 382.80   | 384.17   | 385.19   | 385.54   | 385.73   | 379.86   | 385.77   | 1.66x                        | 1.66x    |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.8 MB/s, tsv-json 9.0 MB/s, tsv-wasm-json 8.4 MB/s, tsv-json-no-locations 14.9 MB/s, tsv-wasm-json-no-locations 13.4 MB/s, tsv-internal 118.8 MB/s, tsv-wasm-internal 81.8 MB/s, rsvelte-parse 4.1 MB/s, rsvelte-parse-skip-expr-loc 6.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.3x tsv-internal, tsv-wasm-json 9.7x tsv-wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier   | 0.18       | 16 | 5.40    | 5.46    | 5.48    | 5.49    | 5.52    | 5.33    | 5.53    | baseline              | baseline |
| tsv        | 13.78      | 55 | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 0.07    | 74.7x                 | 74.5x    |
| tsv-wasm   | 9.74       | 47 | 0.10    | 0.10    | 0.10    | 0.10    | 0.11    | 0.10    | 0.11    | 52.8x                 | 52.7x    |
| oxfmt      | 0.18       | 8  | 5.49    | 5.50    | 5.51    | —       | —       | 5.47    | 5.51    | 0.99x                 | 0.98x    |
| biome-wasm | 0.86       | 7  | 1.16    | 1.17    | 1.18    | —       | —       | 1.15    | 1.20    | 4.67x                 | 4.66x    |

**Files (intersection):** 945

**Throughput:** prettier 0.4 MB/s, tsv 32.1 MB/s, tsv-wasm 22.7 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

**Coverage:** prettier 951/951 (100%), tsv 951/951 (100%), tsv-wasm 951/951 (100%), oxfmt 951/951 (100%), biome-wasm 945/951 (99%)

**Omitted from every row's timed set:** 6 of 951 files, 3.5% of the group's bytes (0.6% of its files) — by row: biome-wasm 6 (6 unsupported_syntax). Each is a reviewed entry in `lib/perf_omit.ts`.

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) | by p50   |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- | -------- |
| acorn-typescript           | 0.28       | 16 | 3.60    | 3.62    | 3.64    | 3.64    | 3.66    | 3.58    | 3.66    | baseline                      | baseline |
| tsv-json                   | 0.47       | 8  | 2.14    | 2.14    | 2.14    | —       | —       | 2.13    | 2.14    | 1.69x                         | 1.68x    |
| tsv-wasm-json              | 0.46       | 8  | 2.19    | 2.20    | 2.20    | —       | —       | 2.18    | 2.20    | 1.64x                         | 1.64x    |
| tsv-json-no-locations      | 0.99       | 7  | 1.01    | 1.01    | 1.01    | —       | —       | 1.01    | 1.01    | 3.57x                         | 3.56x    |
| tsv-wasm-json-no-locations | 0.93       | 8  | 1.08    | 1.08    | 1.08    | —       | —       | 1.08    | 1.09    | 3.33x                         | 3.33x    |
| tsv-internal               | 8.81       | 41 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 31.8x                         | 31.7x    |
| tsv-wasm-internal          | 6.83       | 35 | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 0.15    | 24.6x                         | 24.6x    |
| oxc-parser                 | 0.72       | 8  | 1.40    | 1.40    | 1.41    | —       | —       | 1.38    | 1.42    | 2.58x                         | 2.57x    |
| oxc-parser-wasm            | 0.69       | 8  | 1.44    | 1.44    | 1.45    | —       | —       | 1.43    | 1.45    | 2.50x                         | 2.50x    |
| yuku-parser                | 2.36       | 9  | 0.42    | 0.43    | 0.44    | —       | —       | 0.42    | 0.44    | 8.51x                         | 8.50x    |
| yuku-parser-wasm           | 2.74       | 12 | 0.37    | 0.37    | 0.38    | 0.39    | 0.39    | 0.36    | 0.39    | 9.86x                         | 9.82x    |
| swc                        | 0.55       | 8  | 1.81    | 1.81    | 1.81    | —       | —       | 1.80    | 1.81    | 1.99x                         | 1.99x    |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.1 MB/s, tsv-json 8.6 MB/s, tsv-wasm-json 8.4 MB/s, tsv-json-no-locations 18.2 MB/s, tsv-wasm-json-no-locations 17.1 MB/s, tsv-internal 162.5 MB/s, tsv-wasm-internal 125.9 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.5 MB/s, yuku-parser-wasm 50.4 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv-wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv-wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv-wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.1% of the group's bytes (0.1% of its files) — by row: acorn-typescript 3 (3 tool_limit); oxc-parser 2 (2 harness_path_threading); oxc-parser-wasm 2 (2 harness_path_threading); yuku-parser 2 (2 harness_path_threading); yuku-parser-wasm 2 (2 harness_path_threading); swc 3 (3 tool_limit). Each is a reviewed entry in `lib/perf_omit.ts`.

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 18.8x tsv-internal, tsv-wasm-json 15.0x tsv-wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) | by p50   |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- | -------- |
| prettier    | 0.07       | 14 | 14.89   | 14.97   | 15.05   | 15.11   | 15.12   | 14.75   | 15.12   | baseline              | baseline |
| tsv         | 2.42       | 13 | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 0.41    | 36.0x                 | 36.0x    |
| tsv-wasm    | 1.77       | 9  | 0.57    | 0.57    | 0.57    | —       | —       | 0.56    | 0.57    | 26.4x                 | 26.3x    |
| oxfmt       | 1.07       | 8  | 0.93    | 0.94    | 0.94    | —       | —       | 0.93    | 0.94    | 15.9x                 | 16.0x    |
| biome-wasm  | 0.20       | 6  | 4.90    | 4.91    | 4.92    | —       | —       | 4.89    | 4.95    | 3.04x                 | 3.04x    |
| dprint-wasm | 0.30       | 8  | 3.31    | 3.31    | 3.31    | —       | —       | 3.30    | 3.31    | 4.51x                 | 4.51x    |

**Files (intersection):** 2642

**Throughput:** prettier 1.2 MB/s, tsv 44.6 MB/s, tsv-wasm 32.6 MB/s, oxfmt 19.7 MB/s, biome-wasm 3.8 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv-wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2642/2645 (99%), dprint-wasm 2645/2645 (100%)

**Omitted from every row's timed set:** 3 of 2645 files, 0.0% of the group's bytes (0.1% of its files) — by row: oxfmt 2 (2 harness_path_threading); biome-wasm 3 (3 harness_path_threading). Each is a reviewed entry in `lib/perf_omit.ts`.

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) | by p50   |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- | -------- |
| svelte/compiler   | 81.49      | 366  | 12.17    | 12.59    | 12.80    | 12.95    | 13.50    | 12.00    | 13.92    | baseline                     | baseline |
| tsv-json          | 45.84      | 183  | 21.86    | 21.96    | 24.25    | 24.80    | 24.96    | 21.27    | 25.21    | 0.56x                        | 0.56x    |
| tsv-wasm-json     | 43.18      | 167  | 23.16    | 23.42    | 25.06    | 25.39    | 31.91    | 22.99    | 33.00    | 0.53x                        | 0.53x    |
| tsv-internal      | 255.25     | 1164 | 3.92     | 3.93     | 3.95     | 3.97     | 4.06     | 3.88     | 4.21     | 3.13x                        | 3.11x    |
| tsv-wasm-internal | 171.77     | 759  | 5.82     | 5.83     | 5.86     | 5.90     | 6.03     | 5.79     | 6.10     | 2.11x                        | 2.09x    |
| postcss           | 81.79      | 333  | 12.16    | 12.50    | 12.90    | 13.26    | 13.76    | 12.03    | 14.24    | 1.00x                        | 1.00x    |

**Files (intersection):** 55

**Throughput:** svelte/compiler 33.0 MB/s, tsv-json 18.6 MB/s, tsv-wasm-json 17.5 MB/s, tsv-internal 103.4 MB/s, tsv-wasm-internal 69.6 MB/s, postcss 33.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.6x tsv-internal, tsv-wasm-json 4.0x tsv-wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) | by p50   |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- | -------- |
| prettier   | 1.72       | 14  | 581.14   | 584.98   | 592.56   | 596.96   | 604.48   | 578.61   | 606.36   | baseline              | baseline |
| tsv        | 149.59     | 698 | 6.68     | 6.70     | 6.74     | 6.78     | 7.01     | 6.62     | 9.86     | 87.0x                 | 87.0x    |
| tsv-wasm   | 104.47     | 484 | 9.57     | 9.60     | 9.66     | 9.71     | 9.99     | 9.51     | 12.99    | 60.8x                 | 60.7x    |
| oxfmt      | 52.55      | 260 | 18.99    | 19.36    | 19.82    | 20.14    | 20.82    | 17.66    | 21.63    | 30.6x                 | 30.6x    |
| biome-wasm | 10.45      | 36  | 95.87    | 101.14   | 112.77   | 113.54   | 116.40   | 94.85    | 117.73   | 6.08x                 | 6.06x    |
| malva-wasm | 21.09      | 97  | 47.42    | 47.50    | 47.76    | 48.07    | 48.22    | 47.27    | 51.21    | 12.3x                 | 12.3x    |

**Files (intersection):** 54

**Throughput:** prettier 0.6 MB/s, tsv 54.4 MB/s, tsv-wasm 38.0 MB/s, oxfmt 19.1 MB/s, biome-wasm 3.8 MB/s, malva-wasm 7.7 MB/s

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

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv-wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **74.7x** prettier, **75.7x** oxfmt |
| format typescript (2642f) | **36.0x** prettier, **2.26x** oxfmt |
| format css (54f) | **87.0x** prettier, **2.85x** oxfmt |
| parse svelte (951f) | **2.36x** svelte/compiler, **2.19x** rsvelte-parse |
| parse typescript (2642f) | **1.69x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.85x** swc |
| parse css (55f) | **0.56x** svelte/compiler, **0.56x** postcss |

## Comparisons to tsv-wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (945f) | **52.8x** prettier, **11.3x** biome-wasm |
| format typescript (2642f) | **26.4x** prettier, **8.67x** biome-wasm, **5.85x** dprint-wasm |
| format css (54f) | **60.8x** prettier, **10.0x** biome-wasm, **4.95x** malva-wasm |
| parse svelte (951f) | **2.22x** svelte/compiler |
| parse typescript (2642f) | **1.64x** acorn-typescript, **0.66x** oxc-parser-wasm, **0.17x** yuku-parser-wasm |
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
