# tsv benchmark results

**Runtime:** deno

**Date:** 2026-07-04T16:57:37.990Z — tsv 0.1.0 (ad1c91b6)

**Corpus:** 1263 Svelte (1.8 MB), 4523 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5968 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.35    | 11  | 425.47   | 428.70   | 431.34   | 441.54   | 451.30   | 422.84   | 453.74   | baseline                     |
| tsv-json          | 4.80    | 23  | 208.15   | 209.23   | 210.80   | 212.68   | 216.97   | 206.07   | 218.18   | 2.05x                        |
| tsv_wasm-json     | 4.09    | 21  | 244.61   | 245.24   | 246.70   | 246.86   | 246.96   | 242.11   | 246.98   | 1.74x                        |
| tsv-internal      | 37.97   | 162 | 26.52    | 27.16    | 31.07    | 34.69    | 45.36    | 24.25    | 49.67    | 16.2x                        |
| tsv_wasm-internal | 28.07   | 138 | 35.54    | 35.98    | 36.24    | 36.52    | 37.59    | 35.03    | 45.90    | 12.0x                        |

**Files (intersection):** 1215

**Throughput:** svelte/compiler 4.1 MB/s, tsv-json 8.5 MB/s, tsv_wasm-json 7.2 MB/s, tsv-internal 66.9 MB/s, tsv_wasm-internal 49.4 MB/s

**Coverage:** svelte/compiler 1216/1263 (96%), tsv-json 1225/1263 (96%), tsv_wasm-json 1225/1263 (96%), tsv-internal 1225/1263 (96%), tsv_wasm-internal 1225/1263 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.9x tsv-internal, tsv_wasm-json 6.9x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23    | 7  | 4317.23  | 4371.82  | 4391.77  | —        | —        | 4241.56  | 4410.72  | baseline              |
| tsv        | 11.49   | 50 | 86.91    | 88.49    | 95.01    | 99.66    | 103.08   | 84.71    | 103.19   | 49.8x                 |
| tsv_wasm   | 8.02    | 40 | 124.84   | 125.41   | 126.08   | 126.98   | 132.57   | 122.52   | 135.55   | 34.8x                 |
| oxfmt      | 0.24    | 7  | 4250.74  | 4267.82  | 4282.03  | —        | —        | 4190.43  | 4301.21  | 1.02x                 |
| biome-wasm | 1.26    | 7  | 790.43   | 794.74   | 797.71   | —        | —        | 786.16   | 797.77   | 5.47x                 |

**Files (intersection):** 1214

**Throughput:** prettier 0.4 MB/s, tsv 19.5 MB/s, tsv_wasm 13.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage:** prettier 1224/1263 (96%), tsv 1225/1263 (96%), tsv_wasm 1225/1263 (96%), oxfmt 1221/1263 (96%), biome-wasm 1261/1263 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.35    | 4  | 2.89    | 2.90    | 2.95    | —       | —       | 2.88    | 2.98    | baseline                      |
| tsv-json          | 0.51    | 5  | 1.99    | 1.99    | 2.02    | —       | —       | 1.94    | 2.03    | 1.46x                         |
| tsv_wasm-json     | 0.45    | 4  | 2.20    | 2.21    | 2.22    | —       | —       | 2.20    | 2.22    | 1.31x                         |
| tsv-internal      | 5.79    | 29 | 0.17    | 0.18    | 0.18    | 0.19    | 0.19    | 0.16    | 0.19    | 16.7x                         |
| tsv_wasm-internal | 4.25    | 22 | 0.24    | 0.24    | 0.24    | 0.24    | 0.24    | 0.23    | 0.24    | 12.3x                         |
| oxc-parser        | 0.84    | 5  | 1.19    | 1.21    | 1.21    | —       | —       | 1.18    | 1.22    | 2.42x                         |
| oxc-parser-wasm   | 0.75    | 4  | 1.34    | 1.35    | 1.37    | —       | —       | 1.31    | 1.39    | 2.17x                         |

**Files (intersection):** 4170

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 8.1 MB/s, tsv_wasm-json 7.3 MB/s, tsv-internal 93.3 MB/s, tsv_wasm-internal 68.4 MB/s, oxc-parser 13.5 MB/s, oxc-parser-wasm 12.1 MB/s

**Coverage:** acorn-typescript 4191/4523 (92%), tsv-json 4323/4523 (95%), tsv_wasm-json 4323/4523 (95%), tsv-internal 4323/4523 (95%), tsv_wasm-internal 4323/4523 (95%), oxc-parser 4331/4523 (95%), oxc-parser-wasm 4523/4523 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv_wasm-json 9.4x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.07    | 5 | 13.63   | 14.87   | 16.01   | —       | —       | 13.39   | 17.55   | baseline              |
| tsv        | 1.62    | 9 | 0.61    | 0.62    | 0.62    | —       | —       | 0.61    | 0.62    | 22.4x                 |
| tsv_wasm   | 1.11    | 5 | 0.90    | 0.90    | 0.92    | —       | —       | 0.89    | 0.94    | 15.3x                 |
| oxfmt      | 0.93    | 5 | 1.07    | 1.08    | 1.08    | —       | —       | 1.07    | 1.08    | 12.8x                 |
| biome-wasm | 0.20    | 7 | 5.01    | 5.01    | 5.05    | —       | —       | 4.96    | 5.09    | 2.75x                 |

**Files (intersection):** 4278

**Throughput:** prettier 1.2 MB/s, tsv 26.2 MB/s, tsv_wasm 18.0 MB/s, oxfmt 15.0 MB/s, biome-wasm 3.2 MB/s

**Coverage:** prettier 4357/4523 (96%), tsv 4323/4523 (95%), tsv_wasm 4323/4523 (95%), oxfmt 4334/4523 (95%), biome-wasm 4523/4523 (100%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 197.12  | 713  | 5.06     | 5.59     | 5.74     | 5.83     | 6.07     | 4.84     | 6.63     | baseline                     |
| tsv-json          | 104.40  | 449  | 9.53     | 9.85     | 11.04    | 13.10    | 13.82    | 9.25     | 14.35    | 0.53x                        |
| tsv_wasm-json     | 83.93   | 406  | 11.85    | 12.11    | 12.40    | 12.55    | 13.16    | 11.61    | 14.58    | 0.43x                        |
| tsv-internal      | 254.66  | 1148 | 3.92     | 4.03     | 4.20     | 4.55     | 6.56     | 3.66     | 25.91    | 1.29x                        |
| tsv_wasm-internal | 178.90  | 862  | 5.58     | 5.63     | 5.70     | 5.74     | 5.90     | 5.51     | 7.33     | 0.91x                        |

**Files (intersection):** 147

**Throughput:** svelte/compiler 35.5 MB/s, tsv-json 18.8 MB/s, tsv_wasm-json 15.1 MB/s, tsv-internal 45.9 MB/s, tsv_wasm-internal 32.3 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 153/182 (84%), tsv_wasm-json 153/182 (84%), tsv-internal 153/182 (84%), tsv_wasm-internal 153/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.4x tsv-internal, tsv_wasm-json 2.1x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 3.16    | 15  | 314.02   | 319.49   | 329.05   | 338.91   | 355.33   | 309.47   | 359.43   | baseline              |
| tsv        | 143.49  | 650 | 6.96     | 7.11     | 7.34     | 7.75     | 16.16    | 6.60     | 20.38    | 45.3x                 |
| tsv_wasm   | 96.84   | 459 | 10.30    | 10.43    | 10.63    | 10.77    | 12.13    | 10.13    | 16.87    | 30.6x                 |
| oxfmt      | 3.71    | 18  | 268.63   | 272.13   | 274.25   | 275.24   | 278.54   | 266.53   | 279.37   | 1.17x                 |
| biome-wasm | 15.54   | 78  | 64.27    | 64.96    | 65.45    | 65.80    | 66.36    | 62.77    | 67.16    | 4.91x                 |

**Files (intersection):** 153

**Throughput:** prettier 0.6 MB/s, tsv 27.4 MB/s, tsv_wasm 18.5 MB/s, oxfmt 0.7 MB/s, biome-wasm 3.0 MB/s

**Coverage:** prettier 181/182 (99%), tsv 153/182 (84%), tsv_wasm 153/182 (84%), oxfmt 181/182 (99%), biome-wasm 182/182 (100%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.4 MB | 802.7 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 399.8 KB | 0.4x | 0.5x |
| tsv_wasm | 2.6 MB | 883.7 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 13.2x | 9.3x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.1x | 2.9x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 681.1 KB | 0.4x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1214f) | **49.8x** prettier, **48.9x** oxfmt |
| format typescript (4278f) | **22.4x** prettier, **1.74x** oxfmt |
| format css (153f) | **45.3x** prettier, **38.7x** oxfmt |
| parse svelte (1215f) | **2.05x** svelte |
| parse typescript (4170f) | **1.46x** svelte, **0.60x** oxc-parser |
| parse css (147f) | **0.53x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1214f) | **34.8x** prettier, **6.35x** biome-wasm |
| format typescript (4278f) | **15.3x** prettier, **5.58x** biome-wasm |
| format css (153f) | **30.6x** prettier, **6.23x** biome-wasm |
| parse svelte (1215f) | **1.74x** svelte |
| parse typescript (4170f) | **1.31x** svelte, **0.61x** oxc-parser-wasm |
| parse css (147f) | **0.43x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1308 unique file+error combinations — Svelte 168, TypeScript 1079, CSS 61.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 332
- parse/typescript: tsv-json: 200
- parse/typescript: tsv_wasm-json: 200
- parse/typescript: tsv-internal: 200
- parse/typescript: tsv_wasm-internal: 200
- format/typescript: tsv: 200
- format/typescript: tsv_wasm: 200
- parse/typescript: oxc-parser: 192
- format/typescript: oxfmt: 189
- format/typescript: prettier: 166
- parse/svelte: svelte/compiler: 47
- format/svelte: oxfmt: 42
- format/svelte: prettier: 39
- parse/svelte: tsv-json: 38
- parse/svelte: tsv_wasm-json: 38
- parse/svelte: tsv-internal: 38
- parse/svelte: tsv_wasm-internal: 38
- format/svelte: tsv: 38
- format/svelte: tsv_wasm: 38
- parse/css: svelte/compiler: 30
- parse/css: tsv-json: 29
- parse/css: tsv_wasm-json: 29
- parse/css: tsv-internal: 29
- parse/css: tsv_wasm-internal: 29
- format/css: tsv: 29
- format/css: tsv_wasm: 29
- format/svelte: biome-wasm: 2
- format/css: prettier: 1
- format/css: oxfmt: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
