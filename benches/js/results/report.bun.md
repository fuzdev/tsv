# tsv benchmark results

**Runtime:** bun

**Date:** 2026-07-04T17:17:03.227Z — tsv 0.1.0 (ad1c91b6)

**Corpus:** 1264 Svelte (1.8 MB), 4523 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5969 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 1.17    | 5   | 856.33   | 871.40   | 983.45   | —        | —        | 835.60   | 1093.20  | baseline                     |
| tsv-json          | 4.56    | 24  | 210.23   | 239.37   | 276.89   | 312.97   | 339.18   | 177.84   | 345.55   | 3.89x                        |
| tsv_wasm-json     | 4.90    | 21  | 203.59   | 212.36   | 252.98   | 263.79   | 265.22   | 186.66   | 265.37   | 4.18x                        |
| tsv-internal      | 29.28   | 144 | 33.84    | 36.36    | 39.54    | 41.46    | 43.56    | 26.33    | 94.80    | 25.0x                        |
| tsv_wasm-internal | 12.34   | 41  | 79.67    | 108.56   | 116.81   | 125.63   | 146.52   | 59.54    | 149.26   | 10.5x                        |

**Files (intersection):** 1216

**Throughput:** svelte/compiler 2.1 MB/s, tsv-json 8.0 MB/s, tsv_wasm-json 8.6 MB/s, tsv-internal 51.6 MB/s, tsv_wasm-internal 21.7 MB/s

**Coverage:** svelte/compiler 1217/1264 (96%), tsv-json 1226/1264 (96%), tsv_wasm-json 1226/1264 (96%), tsv-internal 1226/1264 (96%), tsv_wasm-internal 1226/1264 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 6.4x tsv-internal, tsv_wasm-json 2.5x tsv_wasm-internal

## format/svelte

| Task Name | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.21    | 6  | 5.02    | 5.16    | 6.10    | —       | —       | 4.46    | 7.41    | baseline              |
| tsv       | 11.87   | 58 | 0.08    | 0.09    | 0.09    | 0.09    | 0.10    | 0.08    | 0.10    | 57.2x                 |
| tsv_wasm  | 6.52    | 32 | 0.14    | 0.18    | 0.20    | 0.22    | 0.25    | 0.12    | 0.26    | 31.4x                 |
| oxfmt     | 0.18    | 6  | 5.61    | 5.63    | 5.70    | —       | —       | 5.50    | 5.77    | 0.86x                 |

**Files (intersection):** 1217

**Throughput:** prettier 0.4 MB/s, tsv 20.9 MB/s, tsv_wasm 11.5 MB/s, oxfmt 0.3 MB/s

**Coverage:** prettier 1225/1264 (96%), tsv 1226/1264 (96%), tsv_wasm 1226/1264 (96%), oxfmt 1222/1264 (96%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.15    | 6  | 6.49    | 6.53    | 6.63    | —       | —       | 6.41    | 6.73    | baseline                      |
| tsv-json          | 0.72    | 4  | 1.39    | 1.39    | 1.39    | —       | —       | 1.38    | 1.40    | 4.67x                         |
| tsv_wasm-json     | 0.66    | 5  | 1.52    | 1.52    | 1.53    | —       | —       | 1.49    | 1.53    | 4.28x                         |
| tsv-internal      | 7.08    | 35 | 0.14    | 0.14    | 0.14    | 0.15    | 0.15    | 0.14    | 0.16    | 45.8x                         |
| tsv_wasm-internal | 4.84    | 23 | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 31.3x                         |
| oxc-parser        | 1.02    | 5  | 0.99    | 0.99    | 1.00    | —       | —       | 0.98    | 1.01    | 6.57x                         |

**Files (intersection):** 4170

**Throughput:** acorn-typescript 2.5 MB/s, tsv-json 11.6 MB/s, tsv_wasm-json 10.6 MB/s, tsv-internal 113.9 MB/s, tsv_wasm-internal 77.9 MB/s, oxc-parser 16.3 MB/s

**Coverage:** acorn-typescript 4191/4523 (92%), tsv-json 4323/4523 (95%), tsv_wasm-json 4323/4523 (95%), tsv-internal 4323/4523 (95%), tsv_wasm-internal 4323/4523 (95%), oxc-parser 4331/4523 (95%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.8x tsv-internal, tsv_wasm-json 7.3x tsv_wasm-internal

## format/typescript

| Task Name | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.06    | 7 | 16.82   | 16.97   | 17.18   | —       | —       | 16.40   | 17.27   | baseline              |
| tsv       | 1.71    | 9 | 0.58    | 0.59    | 0.59    | —       | —       | 0.57    | 0.60    | 28.8x                 |
| tsv_wasm  | 1.22    | 7 | 0.82    | 0.82    | 0.82    | —       | —       | 0.82    | 0.82    | 20.5x                 |
| oxfmt     | 0.71    | 5 | 1.40    | 1.41    | 1.44    | —       | —       | 1.39    | 1.45    | 11.9x                 |

**Files (intersection):** 4278

**Throughput:** prettier 1.0 MB/s, tsv 27.6 MB/s, tsv_wasm 19.7 MB/s, oxfmt 11.5 MB/s

**Coverage:** prettier 4357/4523 (96%), tsv 4323/4523 (95%), tsv_wasm 4323/4523 (95%), oxfmt 4334/4523 (95%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 134.82  | 516  | 7.38     | 7.70     | 8.67     | 12.27    | 21.64    | 7.14     | 25.58    | baseline                     |
| tsv-json          | 125.44  | 582  | 7.85     | 8.37     | 8.83     | 9.02     | 9.98     | 7.45     | 14.05    | 0.93x                        |
| tsv_wasm-json     | 103.51  | 502  | 9.62     | 9.86     | 10.17    | 10.38    | 13.45    | 9.09     | 14.95    | 0.77x                        |
| tsv-internal      | 298.73  | 1443 | 3.34     | 3.39     | 3.45     | 3.49     | 3.69     | 3.27     | 7.06     | 2.22x                        |
| tsv_wasm-internal | 203.86  | 953  | 4.89     | 4.95     | 5.09     | 5.20     | 5.48     | 4.81     | 7.44     | 1.51x                        |

**Files (intersection):** 147

**Throughput:** svelte/compiler 24.3 MB/s, tsv-json 22.6 MB/s, tsv_wasm-json 18.7 MB/s, tsv-internal 53.9 MB/s, tsv_wasm-internal 36.8 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 153/182 (84%), tsv_wasm-json 153/182 (84%), tsv-internal 153/182 (84%), tsv_wasm-internal 153/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.4x tsv-internal, tsv_wasm-json 2.0x tsv_wasm-internal

## format/css

| Task Name | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 3.01    | 14  | 330.09   | 338.71   | 350.46   | 357.53   | 366.63   | 318.88   | 368.90   | baseline              |
| tsv       | 160.64  | 764 | 6.21     | 6.28     | 6.38     | 6.47     | 6.82     | 6.09     | 7.64     | 53.3x                 |
| tsv_wasm  | 114.40  | 559 | 8.73     | 8.81     | 8.92     | 9.00     | 9.19     | 8.56     | 14.81    | 38.0x                 |
| oxfmt     | 2.74    | 14  | 363.99   | 369.77   | 372.82   | 381.37   | 393.28   | 346.86   | 396.26   | 0.91x                 |

**Files (intersection):** 153

**Throughput:** prettier 0.6 MB/s, tsv 30.6 MB/s, tsv_wasm 21.8 MB/s, oxfmt 0.5 MB/s

**Coverage:** prettier 181/182 (99%), tsv 153/182 (84%), tsv_wasm 153/182 (84%), oxfmt 181/182 (99%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.4 MB | 802.7 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 399.8 KB | 0.4x | 0.5x |
| tsv_wasm | 2.6 MB | 883.7 KB | — | — |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 681.1 KB | 0.4x | 0.5x |
| tsv (napi) | 3.6 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.0x | 2.8x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.7x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.2x | 2.1x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1217f) | **57.2x** prettier, **66.3x** oxfmt |
| format typescript (4278f) | **28.8x** prettier, **2.41x** oxfmt |
| format css (153f) | **53.3x** prettier, **58.6x** oxfmt |
| parse svelte (1216f) | **3.89x** svelte |
| parse typescript (4170f) | **4.67x** svelte, **0.71x** oxc-parser |
| parse css (147f) | **0.93x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1217f) | **31.4x** prettier |
| format typescript (4278f) | **20.5x** prettier |
| format css (153f) | **38.0x** prettier |
| parse svelte (1216f) | **4.18x** svelte |
| parse typescript (4170f) | **4.28x** svelte |
| parse css (147f) | **0.77x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1306 unique file+error combinations — Svelte 166, TypeScript 1079, CSS 61.

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
- format/css: prettier: 1
- format/css: oxfmt: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
