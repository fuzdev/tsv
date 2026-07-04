# tsv benchmark results

**Runtime:** bun

**Date:** 2026-07-03T21:13:04.771Z — tsv 0.1.0 (2136c09e)

**Corpus:** 1263 Svelte (1.8 MB), 4523 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5968 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 1.37    | 7   | 731.76   | 738.84   | 740.39   | —        | —        | 709.54   | 742.00   | baseline                     |
| tsv-json          | 3.47    | 18  | 287.36   | 290.61   | 291.79   | 292.45   | 293.36   | 283.44   | 293.58   | 2.53x                        |
| tsv_wasm-json     | 3.09    | 13  | 325.10   | 326.98   | 353.73   | 356.04   | 359.81   | 320.29   | 360.75   | 2.25x                        |
| tsv-internal      | 46.36   | 221 | 21.51    | 21.75    | 22.00    | 22.19    | 22.59    | 21.21    | 52.99    | 33.7x                        |
| tsv_wasm-internal | 9.31    | 36  | 107.51   | 108.93   | 110.98   | 138.56   | 142.85   | 101.81   | 143.63   | 6.77x                        |

**Files (intersection):** 1215

**Throughput:** svelte/compiler 2.4 MB/s, tsv-json 6.1 MB/s, tsv_wasm-json 5.4 MB/s, tsv-internal 81.4 MB/s, tsv_wasm-internal 16.3 MB/s

**Coverage:** svelte/compiler 1216/1263 (96%), tsv-json 1225/1263 (96%), tsv_wasm-json 1225/1263 (96%), tsv-internal 1225/1263 (96%), tsv_wasm-internal 1225/1263 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.3x tsv-internal, tsv_wasm-json 3.0x tsv_wasm-internal

## format/svelte

| Task Name | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.24    | 7  | 4.09    | 4.17    | 4.18    | —       | —       | 4.02    | 4.20    | baseline              |
| tsv       | 11.38   | 57 | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 0.09    | 46.7x                 |
| tsv_wasm  | 8.40    | 37 | 0.12    | 0.12    | 0.12    | 0.16    | 0.16    | 0.12    | 0.16    | 34.5x                 |
| oxfmt     | 0.18    | 7  | 5.45    | 5.49    | 5.55    | —       | —       | 5.35    | 5.63    | 0.75x                 |

**Files (intersection):** 1216

**Throughput:** prettier 0.4 MB/s, tsv 20.0 MB/s, tsv_wasm 14.7 MB/s, oxfmt 0.3 MB/s

**Coverage:** prettier 1224/1263 (96%), tsv 1225/1263 (96%), tsv_wasm 1225/1263 (96%), oxfmt 1221/1263 (96%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.16    | 7  | 6.12    | 6.18    | 6.22    | —       | —       | 6.05    | 6.28    | baseline                      |
| tsv-json          | 0.74    | 5  | 1.36    | 1.36    | 1.37    | —       | —       | 1.35    | 1.37    | 4.52x                         |
| tsv_wasm-json     | 0.67    | 5  | 1.48    | 1.49    | 1.50    | —       | —       | 1.47    | 1.50    | 4.14x                         |
| tsv-internal      | 6.90    | 35 | 0.14    | 0.15    | 0.15    | 0.15    | 0.15    | 0.14    | 0.15    | 42.4x                         |
| tsv_wasm-internal | 4.51    | 22 | 0.22    | 0.22    | 0.22    | 0.22    | 0.22    | 0.22    | 0.22    | 27.7x                         |
| oxc-parser        | 1.05    | 6  | 0.95    | 0.95    | 0.95    | —       | —       | 0.94    | 0.95    | 6.48x                         |

**Files (intersection):** 4170

**Throughput:** acorn-typescript 2.6 MB/s, tsv-json 11.8 MB/s, tsv_wasm-json 10.8 MB/s, tsv-internal 110.9 MB/s, tsv_wasm-internal 72.4 MB/s, oxc-parser 17.0 MB/s

**Coverage:** acorn-typescript 4191/4523 (92%), tsv-json 4323/4523 (95%), tsv_wasm-json 4323/4523 (95%), tsv-internal 4323/4523 (95%), tsv_wasm-internal 4323/4523 (95%), oxc-parser 4331/4523 (95%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.4x tsv-internal, tsv_wasm-json 6.7x tsv_wasm-internal

## format/typescript

| Task Name | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.06    | 5 | 15.67   | 16.00   | 16.41   | —       | —       | 15.36   | 16.54   | baseline              |
| tsv       | 1.63    | 8 | 0.61    | 0.61    | 0.61    | —       | —       | 0.61    | 0.61    | 25.4x                 |
| tsv_wasm  | 1.20    | 6 | 0.84    | 0.84    | 0.84    | —       | —       | 0.83    | 0.84    | 18.6x                 |
| oxfmt     | 0.72    | 5 | 1.38    | 1.38    | 1.40    | —       | —       | 1.36    | 1.41    | 11.3x                 |

**Files (intersection):** 4278

**Throughput:** prettier 1.0 MB/s, tsv 26.3 MB/s, tsv_wasm 19.3 MB/s, oxfmt 11.7 MB/s

**Coverage:** prettier 4357/4523 (96%), tsv 4323/4523 (95%), tsv_wasm 4323/4523 (95%), oxfmt 4334/4523 (95%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 139.24  | 528  | 7.18     | 7.31     | 7.92     | 19.13    | 27.46    | 6.94     | 29.11    | baseline                     |
| tsv-json          | 130.72  | 579  | 7.57     | 8.02     | 8.35     | 8.46     | 9.57     | 7.33     | 13.34    | 0.94x                        |
| tsv_wasm-json     | 107.39  | 528  | 9.28     | 9.45     | 9.62     | 9.72     | 10.07    | 8.99     | 14.25    | 0.77x                        |
| tsv-internal      | 303.85  | 1255 | 3.29     | 3.31     | 3.33     | 3.35     | 3.41     | 3.26     | 3.65     | 2.18x                        |
| tsv_wasm-internal | 210.11  | 953  | 4.75     | 4.78     | 4.81     | 4.84     | 4.91     | 4.72     | 5.08     | 1.51x                        |

**Files (intersection):** 147

**Throughput:** svelte/compiler 25.1 MB/s, tsv-json 23.6 MB/s, tsv_wasm-json 19.4 MB/s, tsv-internal 54.8 MB/s, tsv_wasm-internal 37.9 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 153/182 (84%), tsv_wasm-json 153/182 (84%), tsv-internal 153/182 (84%), tsv_wasm-internal 153/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.3x tsv-internal, tsv_wasm-json 2.0x tsv_wasm-internal

## format/css

| Task Name | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 3.08    | 15  | 321.21   | 336.85   | 351.47   | 355.78   | 362.98   | 307.46   | 364.78   | baseline              |
| tsv       | 150.40  | 722 | 6.64     | 6.69     | 6.73     | 6.80     | 7.12     | 6.49     | 12.83    | 48.8x                 |
| tsv_wasm  | 112.26  | 549 | 8.90     | 8.97     | 9.04     | 9.11     | 9.28     | 8.76     | 9.50     | 36.4x                 |
| oxfmt     | 3.01    | 14  | 331.90   | 340.24   | 360.32   | 439.71   | 571.87   | 310.95   | 604.91   | 0.98x                 |

**Files (intersection):** 153

**Throughput:** prettier 0.6 MB/s, tsv 28.7 MB/s, tsv_wasm 21.4 MB/s, oxfmt 0.6 MB/s

**Coverage:** prettier 181/182 (99%), tsv 153/182 (84%), tsv_wasm 153/182 (84%), oxfmt 181/182 (99%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 778.5 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.2 MB | 436.6 KB | 0.5x | 0.5x |
| tsv_wasm | 2.6 MB | 894.3 KB | — | — |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.4 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.0 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 730.7 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.0x | 2.9x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1216f) | **46.7x** prettier, **62.1x** oxfmt |
| format typescript (4278f) | **25.4x** prettier, **2.25x** oxfmt |
| format css (153f) | **48.8x** prettier, **50.0x** oxfmt |
| parse svelte (1215f) | **2.53x** svelte |
| parse typescript (4170f) | **4.52x** svelte, **0.70x** oxc-parser |
| parse css (147f) | **0.94x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1216f) | **34.5x** prettier |
| format typescript (4278f) | **18.6x** prettier |
| format css (153f) | **36.4x** prettier |
| parse svelte (1215f) | **2.25x** svelte |
| parse typescript (4170f) | **4.14x** svelte |
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
