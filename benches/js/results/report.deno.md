# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.8.3

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-16T11:34:23.572Z — tsv 0.1.0 (135b7b93)

**Corpus:** 763 Svelte (1.9 MB), 2437 TypeScript (16.8 MB), 49 CSS (0.3 MB) — 3249 files, 19.0 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.59.0, @biomejs/wasm-bundler@2.5.4

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.32       | 10  | 430.50   | 438.30   | 448.23   | 452.65   | 455.96   | 425.16   | 456.79   | baseline                     |
| tsv-json                   | 5.08       | 21  | 196.71   | 198.55   | 201.37   | 201.65   | 202.22   | 195.76   | 202.40   | 2.19x                        |
| tsv-json-no-locations      | 7.92       | 33  | 125.96   | 128.41   | 129.55   | 129.78   | 129.96   | 125.08   | 130.04   | 3.41x                        |
| tsv_wasm-json              | 4.16       | 21  | 239.74   | 241.92   | 243.44   | 245.01   | 245.23   | 237.73   | 245.29   | 1.79x                        |
| tsv_wasm-json-no-locations | 6.30       | 29  | 158.40   | 159.86   | 161.96   | 162.47   | 164.57   | 157.44   | 165.29   | 2.71x                        |
| tsv-internal               | 47.05      | 216 | 21.13    | 21.62    | 21.80    | 21.95    | 22.12    | 20.93    | 22.16    | 20.3x                        |
| tsv_wasm-internal          | 30.47      | 126 | 32.70    | 33.56    | 33.78    | 33.86    | 34.02    | 32.45    | 34.20    | 13.1x                        |

**Files (intersection):** 763

**Throughput:** svelte/compiler 4.4 MB/s, tsv-json 9.5 MB/s, tsv-json-no-locations 14.9 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 11.8 MB/s, tsv-internal 88.4 MB/s, tsv_wasm-internal 57.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.3x tsv-internal, tsv_wasm-json 7.3x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.24       | 7  | 4211.36  | 4222.87  | 4269.79  | —        | —        | 4123.07  | 4329.00  | baseline              |
| tsv        | 14.14      | 70 | 70.39    | 71.73    | 72.51    | 72.69    | 73.65    | 69.69    | 74.57    | 59.4x                 |
| tsv_wasm   | 9.06       | 44 | 109.95   | 111.62   | 113.50   | 113.65   | 114.07   | 108.87   | 114.07   | 38.1x                 |
| oxfmt      | 0.24       | 4  | 4175.27  | 4177.44  | 4194.89  | —        | —        | 4160.93  | 4206.53  | 1.01x                 |
| biome-wasm | 1.43       | 6  | 698.98   | 704.22   | 718.24   | —        | —        | 696.46   | 726.34   | 6.02x                 |

**Files (intersection):** 763

**Throughput:** prettier 0.4 MB/s, tsv 26.5 MB/s, tsv_wasm 17.0 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.35       | 5  | 2.84    | 2.85    | 2.85    | —       | —       | 2.83    | 2.85    | baseline                      |
| tsv-json                   | 0.55       | 4  | 1.81    | 1.82    | 1.82    | —       | —       | 1.80    | 1.83    | 1.57x                         |
| tsv-json-no-locations      | 1.15       | 5  | 0.87    | 0.87    | 0.88    | —       | —       | 0.87    | 0.88    | 3.25x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.11    | 2.12    | 2.13    | —       | —       | 2.09    | 2.13    | 1.35x                         |
| tsv_wasm-json-no-locations | 0.94       | 5  | 1.06    | 1.06    | 1.06    | —       | —       | 1.06    | 1.06    | 2.67x                         |
| tsv-internal               | 7.59       | 29 | 0.13    | 0.13    | 0.13    | 0.14    | 0.14    | 0.13    | 0.14    | 21.6x                         |
| tsv_wasm-internal          | 4.84       | 20 | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 13.8x                         |
| oxc-parser                 | 0.86       | 5  | 1.16    | 1.17    | 1.17    | —       | —       | 1.14    | 1.18    | 2.45x                         |
| oxc-parser-wasm            | 0.77       | 5  | 1.30    | 1.30    | 1.30    | —       | —       | 1.30    | 1.30    | 2.18x                         |

**Files (intersection):** 2434

**Throughput:** acorn-typescript 5.9 MB/s, tsv-json 9.3 MB/s, tsv-json-no-locations 19.2 MB/s, tsv_wasm-json 8.0 MB/s, tsv_wasm-json-no-locations 15.8 MB/s, tsv-internal 127.5 MB/s, tsv_wasm-internal 81.4 MB/s, oxc-parser 14.5 MB/s, oxc-parser-wasm 12.9 MB/s

**Coverage:** acorn-typescript 2434/2437 (99%), tsv-json 2437/2437 (100%), tsv-json-no-locations 2437/2437 (100%), tsv_wasm-json 2437/2437 (100%), tsv_wasm-json-no-locations 2437/2437 (100%), tsv-internal 2437/2437 (100%), tsv_wasm-internal 2437/2437 (100%), oxc-parser 2435/2437 (99%), oxc-parser-wasm 2435/2437 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.7x tsv-internal, tsv_wasm-json 10.2x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 7 | 12069.65 | 12167.69 | 12260.18 | —        | —        | 11993.04 | 12260.74 | baseline              |
| tsv        | 1.99       | 7 | 502.42   | 504.75   | 510.35   | —        | —        | 501.21   | 513.71   | 24.1x                 |
| tsv_wasm   | 1.27       | 6 | 785.30   | 788.84   | 793.57   | —        | —        | 784.25   | 799.75   | 15.4x                 |
| oxfmt      | 1.20       | 7 | 831.29   | 835.78   | 838.50   | —        | —        | 819.38   | 838.50   | 14.6x                 |
| biome-wasm | 0.24       | 5 | 4094.36  | 4109.33  | 4111.69  | —        | —        | 4086.37  | 4113.27  | 2.95x                 |

**Files (intersection):** 2435

**Throughput:** prettier 1.4 MB/s, tsv 33.5 MB/s, tsv_wasm 21.4 MB/s, oxfmt 20.2 MB/s, biome-wasm 4.1 MB/s

**Coverage:** prettier 2437/2437 (100%), tsv 2437/2437 (100%), tsv_wasm 2437/2437 (100%), oxfmt 2435/2437 (99%), biome-wasm 2437/2437 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 99.28      | 488  | 10.02    | 10.27    | 10.53    | 10.72    | 11.39    | 9.66     | 12.49    | baseline                     |
| tsv-json          | 66.59      | 302  | 14.93    | 15.28    | 15.63    | 16.36    | 17.27    | 14.72    | 26.24    | 0.67x                        |
| tsv_wasm-json     | 52.33      | 232  | 18.98    | 19.45    | 20.00    | 20.25    | 20.98    | 18.74    | 21.30    | 0.53x                        |
| tsv-internal      | 304.78     | 1318 | 3.28     | 3.30     | 3.36     | 3.38     | 3.41     | 3.25     | 3.57     | 3.07x                        |
| tsv_wasm-internal | 184.76     | 763  | 5.41     | 5.46     | 5.51     | 5.54     | 5.57     | 5.38     | 6.29     | 1.86x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 32.5 MB/s, tsv-json 21.8 MB/s, tsv_wasm-json 17.1 MB/s, tsv-internal 99.8 MB/s, tsv_wasm-internal 60.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.6x tsv-internal, tsv_wasm-json 3.5x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.93       | 9   | 517.38   | 519.90   | 525.87   | —        | —        | 511.36   | 534.70   | baseline              |
| tsv        | 148.60     | 580 | 6.72     | 6.83     | 7.00     | 7.23     | 7.41     | 6.67     | 8.00     | 76.9x                 |
| tsv_wasm   | 88.45      | 441 | 11.28    | 11.44    | 11.56    | 11.61    | 11.73    | 11.11    | 12.37    | 45.7x                 |
| oxfmt      | 56.36      | 280 | 17.76    | 18.12    | 18.37    | 18.55    | 19.11    | 16.28    | 20.15    | 29.1x                 |
| biome-wasm | 10.27      | 41  | 97.37    | 99.21    | 100.44   | 101.33   | 101.97   | 96.35    | 102.00   | 5.31x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 48.7 MB/s, tsv_wasm 29.0 MB/s, oxfmt 18.5 MB/s, biome-wasm 3.4 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 794.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 389.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 875.4 KB | — | — |
| biome (wasm) | 38.6 MB | 9.3 MB | 15.5x | 10.6x |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.4 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.5 MB | 3.3x | 3.1x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 705.0 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.6x |
| oxfmt (napi) | 8.9 MB | 3.6 MB | 2.6x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **59.4x** prettier, **59.0x** oxfmt |
| format typescript (2435f) | **24.1x** prettier, **1.65x** oxfmt |
| format css (49f) | **76.9x** prettier, **2.64x** oxfmt |
| parse svelte (763f) | **2.19x** svelte |
| parse typescript (2434f) | **1.57x** svelte, **0.64x** oxc-parser |
| parse css (49f) | **0.67x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **38.1x** prettier, **6.33x** biome-wasm |
| format typescript (2435f) | **15.4x** prettier, **5.21x** biome-wasm |
| format css (49f) | **45.7x** prettier, **8.61x** biome-wasm |
| parse svelte (763f) | **1.79x** svelte |
| parse typescript (2434f) | **1.35x** svelte, **0.62x** oxc-parser-wasm |
| parse css (49f) | **0.53x** svelte |

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
