# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-22T09:18:34.995Z — tsv 0.1.0 (d063479a)

**Corpus:** 767 Svelte (1.9 MB), 2448 TypeScript (17.0 MB), 50 CSS (0.3 MB) — 3265 files, 19.2 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (74), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (34), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.5, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.60.0, @biomejs/wasm-bundler@2.5.4, @dprint/typescript@0.96.1

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.22       | 10  | 449.99   | 454.09   | 460.78   | 464.26   | 467.31   | 446.84   | 468.07   | baseline                     |
| tsv-json                   | 4.65       | 19  | 215.11   | 216.64   | 221.47   | 222.24   | 222.39   | 214.04   | 222.41   | 2.09x                        |
| tsv-json-no-locations      | 7.30       | 31  | 137.10   | 138.02   | 141.70   | 142.32   | 142.78   | 134.88   | 142.86   | 3.29x                        |
| tsv_wasm-json              | 4.17       | 17  | 239.84   | 242.04   | 244.29   | 245.78   | 245.96   | 238.93   | 246.01   | 1.88x                        |
| tsv_wasm-json-no-locations | 6.45       | 26  | 155.16   | 156.26   | 159.28   | 159.74   | 160.01   | 153.71   | 160.05   | 2.90x                        |
| tsv-internal               | 49.19      | 226 | 20.20    | 20.71    | 20.87    | 20.97    | 21.05    | 20.00    | 21.25    | 22.1x                        |
| tsv_wasm-internal          | 36.19      | 128 | 27.61    | 28.25    | 28.46    | 28.53    | 28.61    | 27.48    | 28.76    | 16.3x                        |

**Files (intersection):** 767

**Throughput:** svelte/compiler 4.2 MB/s, tsv-json 8.8 MB/s, tsv-json-no-locations 13.9 MB/s, tsv_wasm-json 7.9 MB/s, tsv_wasm-json-no-locations 12.3 MB/s, tsv-internal 93.5 MB/s, tsv_wasm-internal 68.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.6x tsv-internal, tsv_wasm-json 8.7x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4380.68  | 4412.69  | 4435.17  | —        | —        | 4355.17  | 4448.31  | baseline              |
| tsv        | 14.70      | 69 | 67.66    | 69.13    | 69.62    | 69.75    | 71.03    | 67.19    | 73.85    | 64.6x                 |
| tsv_wasm   | 10.28      | 48 | 96.98    | 98.71    | 99.79    | 100.75   | 100.99   | 96.11    | 101.04   | 45.1x                 |
| oxfmt      | 0.23       | 7  | 4312.35  | 4375.55  | 4393.74  | —        | —        | 4256.95  | 4405.10  | 1.01x                 |
| biome-wasm | 1.10       | 6  | 907.74   | 917.71   | 923.12   | —        | —        | 892.63   | 927.18   | 4.83x                 |

**Files (intersection):** 767

**Throughput:** prettier 0.4 MB/s, tsv 28.0 MB/s, tsv_wasm 19.5 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.32       | 5  | 3.13    | 3.13    | 3.13    | —       | —       | 3.13    | 3.13    | baseline                      |
| tsv-json                   | 0.49       | 4  | 2.04    | 2.04    | 2.04    | —       | —       | 2.03    | 2.04    | 1.54x                         |
| tsv-json-no-locations      | 0.99       | 5  | 1.01    | 1.01    | 1.01    | —       | —       | 1.00    | 1.01    | 3.11x                         |
| tsv_wasm-json              | 0.46       | 5  | 2.16    | 2.16    | 2.17    | —       | —       | 2.15    | 2.17    | 1.45x                         |
| tsv_wasm-json-no-locations | 0.92       | 3  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 2.87x                         |
| tsv-internal               | 7.14       | 36 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 22.3x                         |
| tsv_wasm-internal          | 5.60       | 21 | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 17.5x                         |
| oxc-parser                 | 0.77       | 5  | 1.30    | 1.30    | 1.30    | —       | —       | 1.29    | 1.30    | 2.41x                         |
| oxc-parser-wasm            | 0.74       | 5  | 1.34    | 1.35    | 1.35    | —       | —       | 1.34    | 1.35    | 2.33x                         |

**Files (intersection):** 2445

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.3 MB/s, tsv-json-no-locations 16.8 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 15.5 MB/s, tsv-internal 121.0 MB/s, tsv_wasm-internal 94.8 MB/s, oxc-parser 13.1 MB/s, oxc-parser-wasm 12.6 MB/s

**Coverage:** acorn-typescript 2445/2448 (99%), tsv-json 2448/2448 (100%), tsv-json-no-locations 2448/2448 (100%), tsv_wasm-json 2448/2448 (100%), tsv_wasm-json-no-locations 2448/2448 (100%), tsv-internal 2448/2448 (100%), tsv_wasm-internal 2448/2448 (100%), oxc-parser 2446/2448 (99%), oxc-parser-wasm 2446/2448 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.5x tsv-internal, tsv_wasm-json 12.1x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 6 | 13.43   | 13.45   | 13.48   | —       | —       | 13.40   | 13.53   | baseline              |
| tsv         | 1.97       | 8 | 0.51    | 0.51    | 0.52    | —       | —       | 0.51    | 0.52    | 26.5x                 |
| tsv_wasm    | 1.44       | 7 | 0.69    | 0.69    | 0.70    | —       | —       | 0.69    | 0.72    | 19.3x                 |
| oxfmt       | 1.19       | 5 | 0.84    | 0.84    | 0.85    | —       | —       | 0.84    | 0.85    | 15.9x                 |
| biome-wasm  | 0.22       | 3 | 4.50    | 7.30    | 9.61    | —       | —       | 4.49    | 11.15   | 2.99x                 |
| dprint-wasm | 0.32       | 5 | 3.11    | 3.11    | 3.11    | —       | —       | 3.09    | 3.11    | 4.33x                 |

**Files (intersection):** 2446

**Throughput:** prettier 1.3 MB/s, tsv 33.4 MB/s, tsv_wasm 24.4 MB/s, oxfmt 20.1 MB/s, biome-wasm 3.8 MB/s, dprint-wasm 5.5 MB/s

**Coverage:** prettier 2448/2448 (100%), tsv 2448/2448 (100%), tsv_wasm 2448/2448 (100%), oxfmt 2446/2448 (99%), biome-wasm 2448/2448 (100%), dprint-wasm 2448/2448 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 109.60     | 509  | 9.04     | 9.42     | 9.89     | 10.23    | 14.18    | 8.75     | 18.98    | baseline                     |
| tsv-json          | 57.75      | 259  | 17.27    | 17.69    | 17.92    | 19.57    | 20.66    | 16.63    | 25.34    | 0.53x                        |
| tsv_wasm-json     | 55.29      | 244  | 18.02    | 18.36    | 19.28    | 19.76    | 22.99    | 17.68    | 23.77    | 0.50x                        |
| tsv-internal      | 301.56     | 1362 | 3.31     | 3.34     | 3.40     | 3.45     | 3.51     | 3.26     | 4.00     | 2.75x                        |
| tsv_wasm-internal | 211.10     | 954  | 4.73     | 4.77     | 4.83     | 4.86     | 4.91     | 4.66     | 5.06     | 1.93x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 36.3 MB/s, tsv-json 19.1 MB/s, tsv_wasm-json 18.3 MB/s, tsv-internal 99.8 MB/s, tsv_wasm-internal 69.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.2x tsv-internal, tsv_wasm-json 3.8x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.79       | 9   | 549.54   | 565.60   | 578.08   | —        | —        | 539.65   | 579.50   | baseline              |
| tsv        | 147.62     | 627 | 6.77     | 6.83     | 7.00     | 7.06     | 7.34     | 6.70     | 10.76    | 82.3x                 |
| tsv_wasm   | 102.68     | 501 | 9.71     | 9.82     | 9.91     | 9.97     | 10.20    | 9.55     | 13.43    | 57.2x                 |
| oxfmt      | 52.86      | 261 | 18.89    | 19.35    | 19.80    | 20.01    | 21.14    | 17.22    | 21.63    | 29.5x                 |
| biome-wasm | 7.54       | 31  | 133.18   | 142.05   | 179.33   | 187.46   | 211.40   | 123.86   | 212.79   | 4.21x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 48.9 MB/s, tsv_wasm 34.0 MB/s, oxfmt 17.5 MB/s, biome-wasm 2.5 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 787.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 388.7 KB | 0.4x | 0.4x |
| tsv_wasm | 2.4 MB | 867.8 KB | — | — |
| biome (wasm) | 38.6 MB | 9.3 MB | 15.8x | 10.7x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.3x |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 703.2 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.5 MB | 3.2x | 3.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.6x |
| oxfmt (napi) | 8.8 MB | 3.6 MB | 2.5x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (767f) | **64.6x** prettier, **63.6x** oxfmt |
| format typescript (2446f) | **26.5x** prettier, **1.66x** oxfmt |
| format css (50f) | **82.3x** prettier, **2.79x** oxfmt |
| parse svelte (767f) | **2.09x** svelte |
| parse typescript (2445f) | **1.54x** svelte, **0.64x** oxc-parser |
| parse css (50f) | **0.53x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (767f) | **45.1x** prettier, **9.34x** biome-wasm |
| format typescript (2446f) | **19.3x** prettier, **6.47x** biome-wasm, **4.47x** dprint-wasm |
| format css (50f) | **57.2x** prettier, **13.6x** biome-wasm |
| parse svelte (767f) | **1.88x** svelte |
| parse typescript (2445f) | **1.45x** svelte, **0.62x** oxc-parser-wasm |
| parse css (50f) | **0.50x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

7 unique file+error combinations — Svelte 0, TypeScript 7, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
