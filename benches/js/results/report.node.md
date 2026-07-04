# tsv benchmark results

**Runtime:** node

**Date:** 2026-07-04T17:07:43.168Z — tsv 0.1.0 (ad1c91b6)

**Corpus:** 1264 Svelte (1.8 MB), 4523 TypeScript (16.2 MB), 182 CSS (0.2 MB) — 5969 files, 18.2 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.21    | 12  | 454.05   | 455.77   | 457.91   | 459.77   | 461.59   | 441.53   | 462.04   | baseline                     |
| tsv-json          | 4.53    | 21  | 220.59   | 223.25   | 227.11   | 235.64   | 240.55   | 216.19   | 241.69   | 2.05x                        |
| tsv_wasm-json     | 4.05    | 21  | 246.82   | 248.31   | 248.85   | 249.34   | 249.51   | 242.49   | 249.56   | 1.83x                        |
| tsv-internal      | 43.48   | 217 | 22.92    | 23.30    | 23.73    | 23.95    | 24.56    | 21.68    | 26.74    | 19.7x                        |
| tsv_wasm-internal | 23.93   | 118 | 46.68    | 47.79    | 48.99    | 50.41    | 58.18    | 31.46    | 73.95    | 10.8x                        |

**Files (intersection):** 1216

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 8.0 MB/s, tsv_wasm-json 7.1 MB/s, tsv-internal 76.6 MB/s, tsv_wasm-internal 42.1 MB/s

**Coverage:** svelte/compiler 1217/1264 (96%), tsv-json 1226/1264 (96%), tsv_wasm-json 1226/1264 (96%), tsv-internal 1226/1264 (96%), tsv_wasm-internal 1226/1264 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.6x tsv-internal, tsv_wasm-json 5.9x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22    | 6  | 4592.88  | 4607.14  | 4654.19  | —        | —        | 4481.21  | 4714.16  | baseline              |
| tsv        | 12.90   | 64 | 77.39    | 77.92    | 78.30    | 78.75    | 79.92    | 76.73    | 81.52    | 59.3x                 |
| tsv_wasm   | 7.22    | 35 | 136.72   | 142.16   | 144.76   | 146.88   | 152.31   | 133.45   | 154.48   | 33.2x                 |
| oxfmt      | 0.23    | 6  | 4292.04  | 4303.75  | 4319.94  | —        | —        | 4286.96  | 4336.80  | 1.07x                 |
| biome-wasm | 1.43    | 8  | 699.31   | 701.74   | 703.57   | —        | —        | 692.76   | 703.61   | 6.57x                 |

**Files (intersection):** 1215

**Throughput:** prettier 0.4 MB/s, tsv 21.9 MB/s, tsv_wasm 12.3 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.4 MB/s

**Coverage:** prettier 1225/1264 (96%), tsv 1226/1264 (96%), tsv_wasm 1226/1264 (96%), oxfmt 1222/1264 (96%), biome-wasm 1262/1264 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript  | 0.32    | 3  | 3.12    | 3.12    | 3.12    | —       | —       | 3.12    | 3.12    | baseline                      |
| tsv-json          | 0.47    | 4  | 2.12    | 2.12    | 2.13    | —       | —       | 2.12    | 2.13    | 1.47x                         |
| tsv_wasm-json     | 0.44    | 5  | 2.28    | 2.29    | 2.30    | —       | —       | 2.25    | 2.30    | 1.37x                         |
| tsv-internal      | 5.71    | 29 | 0.17    | 0.18    | 0.19    | 0.20    | 0.20    | 0.16    | 0.20    | 17.8x                         |
| tsv_wasm-internal | 4.76    | 22 | 0.21    | 0.21    | 0.22    | 0.22    | 0.23    | 0.20    | 0.23    | 14.9x                         |
| oxc-parser        | 0.74    | 5  | 1.36    | 1.38    | 1.38    | —       | —       | 1.31    | 1.39    | 2.30x                         |
| oxc-parser-wasm   | 0.72    | 4  | 1.38    | 1.38    | 1.38    | —       | —       | 1.38    | 1.39    | 2.26x                         |

**Files (intersection):** 4170

**Throughput:** acorn-typescript 5.2 MB/s, tsv-json 7.6 MB/s, tsv_wasm-json 7.1 MB/s, tsv-internal 92.0 MB/s, tsv_wasm-internal 76.7 MB/s, oxc-parser 11.9 MB/s, oxc-parser-wasm 11.7 MB/s

**Coverage:** acorn-typescript 4191/4523 (92%), tsv-json 4323/4523 (95%), tsv_wasm-json 4323/4523 (95%), tsv-internal 4323/4523 (95%), tsv_wasm-internal 4323/4523 (95%), oxc-parser 4331/4523 (95%), oxc-parser-wasm 4523/4523 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.1x tsv-internal, tsv_wasm-json 10.9x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.07    | 7 | 14.84   | 15.00   | 15.11   | —       | —       | 14.63   | 15.14   | baseline              |
| tsv        | 1.72    | 9 | 0.58    | 0.58    | 0.58    | —       | —       | 0.58    | 0.58    | 25.6x                 |
| tsv_wasm   | 1.25    | 6 | 0.80    | 0.80    | 0.81    | —       | —       | 0.80    | 0.82    | 18.6x                 |
| oxfmt      | 0.96    | 4 | 1.03    | 1.05    | 1.05    | —       | —       | 1.03    | 1.06    | 14.3x                 |
| biome-wasm | 0.23    | 5 | 4.34    | 4.36    | 4.36    | —       | —       | 4.33    | 4.36    | 3.42x                 |

**Files (intersection):** 4278

**Throughput:** prettier 1.1 MB/s, tsv 27.8 MB/s, tsv_wasm 20.2 MB/s, oxfmt 15.6 MB/s, biome-wasm 3.7 MB/s

**Coverage:** prettier 4357/4523 (96%), tsv 4323/4523 (95%), tsv_wasm 4323/4523 (95%), oxfmt 4334/4523 (95%), biome-wasm 4523/4523 (100%)

## parse/css

| Task Name         | ops/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 170.35  | 803  | 5.83     | 6.06     | 6.47     | 6.69     | 7.52     | 5.40     | 10.47    | baseline                     |
| tsv-json          | 106.09  | 488  | 9.37     | 9.68     | 10.06    | 10.45    | 10.94    | 9.08     | 11.55    | 0.62x                        |
| tsv_wasm-json     | 87.99   | 418  | 11.31    | 11.60    | 11.89    | 12.37    | 13.10    | 10.96    | 13.81    | 0.52x                        |
| tsv-internal      | 285.65  | 1307 | 3.49     | 3.56     | 3.70     | 3.87     | 4.28     | 3.40     | 7.47     | 1.68x                        |
| tsv_wasm-internal | 189.62  | 860  | 5.26     | 5.42     | 5.72     | 6.15     | 9.83     | 4.97     | 10.96    | 1.11x                        |

**Files (intersection):** 147

**Throughput:** svelte/compiler 30.7 MB/s, tsv-json 19.1 MB/s, tsv_wasm-json 15.9 MB/s, tsv-internal 51.5 MB/s, tsv_wasm-internal 34.2 MB/s

**Coverage:** svelte/compiler 152/182 (83%), tsv-json 153/182 (84%), tsv_wasm-json 153/182 (84%), tsv-internal 153/182 (84%), tsv_wasm-internal 153/182 (84%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.7x tsv-internal, tsv_wasm-json 2.2x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.21    | 12  | 451.40   | 537.01   | 593.16   | 605.42   | 611.81   | 333.09   | 613.41   | baseline              |
| tsv        | 106.17  | 531 | 9.13     | 10.77    | 12.29    | 13.23    | 15.36    | 6.46     | 16.73    | 47.9x                 |
| tsv_wasm   | 108.53  | 510 | 9.19     | 9.32     | 9.50     | 9.73     | 10.25    | 8.97     | 10.47    | 49.0x                 |
| oxfmt      | 3.20    | 16  | 313.78   | 321.24   | 323.11   | 326.37   | 333.40   | 284.58   | 335.16   | 1.44x                 |
| biome-wasm | 16.79   | 80  | 59.50    | 60.23    | 61.29    | 61.80    | 69.68    | 57.78    | 75.83    | 7.58x                 |

**Files (intersection):** 153

**Throughput:** prettier 0.4 MB/s, tsv 20.2 MB/s, tsv_wasm 20.7 MB/s, oxfmt 0.6 MB/s, biome-wasm 3.2 MB/s

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
| format svelte (1215f) | **59.3x** prettier, **55.4x** oxfmt |
| format typescript (4278f) | **25.6x** prettier, **1.78x** oxfmt |
| format css (153f) | **47.9x** prettier, **33.2x** oxfmt |
| parse svelte (1216f) | **2.05x** svelte |
| parse typescript (4170f) | **1.47x** svelte, **0.64x** oxc-parser |
| parse css (147f) | **0.62x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (1215f) | **33.2x** prettier, **5.05x** biome-wasm |
| format typescript (4278f) | **18.6x** prettier, **5.43x** biome-wasm |
| format css (153f) | **49.0x** prettier, **6.46x** biome-wasm |
| parse svelte (1216f) | **1.83x** svelte |
| parse typescript (4170f) | **1.37x** svelte, **0.61x** oxc-parser-wasm |
| parse css (147f) | **0.52x** svelte |

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
