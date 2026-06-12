# tsv benchmark results

**Date:** 2026-06-12T09:22:00.588Z (9b5c4e98)

**Corpus:** 1240 Svelte (1.6 MB), 4025 TypeScript (13.1 MB), 193 CSS (0.7 MB) — 5458 files, 15.4 MB total

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.8.3, prettier-plugin-svelte@3.5.2, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 2.83    | 15  | 351.79   | 355.71   | 358.76   | 359.83   | 361.46   | 349.59   | 361.86   | baseline                     |
| tsv-json          | 2.06    | 9   | 485.09   | 487.64   | 491.66   | —        | —        | 481.78   | 496.86   | 0.73x                        |
| tsv_wasm-json     | 1.94    | 7   | 516.07   | 517.32   | 519.26   | —        | —        | 515.56   | 522.28   | 0.69x                        |
| tsv-internal      | 27.00   | 119 | 36.82    | 38.03    | 39.75    | 49.37    | 70.56    | 35.49    | 73.56    | 9.55x                        |
| tsv_wasm-internal | 22.10   | 102 | 45.24    | 45.35    | 45.45    | 45.62    | 45.69    | 45.09    | 45.72    | 7.81x                        |

**Files (intersection):** 1192

**Throughput:** svelte/compiler 4.5 MB/s, tsv-json 3.3 MB/s, tsv_wasm-json 3.1 MB/s, tsv-internal 43.4 MB/s, tsv_wasm-internal 35.5 MB/s

**Coverage:** svelte/compiler 1193/1240 (96%), tsv-json 1202/1240 (96%), tsv_wasm-json 1202/1240 (96%), tsv-internal 1202/1240 (96%), tsv_wasm-internal 1202/1240 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.1x tsv-internal, tsv_wasm-json 11.4x tsv_wasm-internal

## format/svelte

| Task Name  | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.32    | 5  | 3153.33  | 3161.18  | 3191.38  | —        | —        | 3116.68  | 3211.52  | baseline              |
| tsv        | 3.94    | 18 | 250.90   | 261.50   | 270.31   | 291.69   | 299.63   | 247.14   | 301.61   | 12.4x                 |
| tsv_wasm   | 2.87    | 12 | 348.41   | 349.01   | 351.97   | 353.36   | 355.47   | 347.71   | 356.00   | 9.06x                 |
| oxfmt      | 0.30    | 5  | 3355.47  | 3380.46  | 3387.86  | —        | —        | 3336.32  | 3392.79  | 0.94x                 |
| biome-wasm | 1.53    | 8  | 651.35   | 652.78   | 655.60   | —        | —        | 646.29   | 658.53   | 4.84x                 |

**Files (intersection):** 1194

**Throughput:** prettier 0.5 MB/s, tsv 6.1 MB/s, tsv_wasm 4.5 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.4 MB/s

**Coverage:** prettier 1201/1240 (96%), tsv 1202/1240 (96%), tsv_wasm 1202/1240 (96%), oxfmt 1201/1240 (96%), biome-wasm 1238/1240 (99%)

## parse/typescript

| Task Name         | ops/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| ----------------- | ------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript  | 0.45    | 5  | 2212.10  | 2213.41  | 2218.80  | —        | —        | 2209.49  | 2222.40  | baseline                      |
| tsv-json          | 0.49    | 5  | 2046.28  | 2052.48  | 2062.33  | —        | —        | 2036.38  | 2068.89  | 1.08x                         |
| tsv_wasm-json     | 0.45    | 5  | 2223.81  | 2224.51  | 2224.64  | —        | —        | 2222.33  | 2224.73  | 1.00x                         |
| tsv-internal      | 4.23    | 16 | 235.99   | 239.29   | 262.67   | 263.12   | 263.95   | 234.97   | 264.16   | 9.37x                         |
| tsv_wasm-internal | 3.42    | 16 | 292.65   | 292.77   | 293.67   | 294.71   | 294.96   | 292.27   | 295.03   | 7.56x                         |
| oxc-parser        | 1.16    | 6  | 863.97   | 878.83   | 884.91   | —        | —        | 845.08   | 890.88   | 2.56x                         |
| oxc-parser-wasm   | 1.02    | 6  | 975.59   | 979.11   | 982.23   | —        | —        | 971.97   | 984.50   | 2.27x                         |

**Files (intersection):** 3603

**Throughput:** acorn-typescript 5.8 MB/s, tsv-json 6.3 MB/s, tsv_wasm-json 5.8 MB/s, tsv-internal 54.8 MB/s, tsv_wasm-internal 44.2 MB/s, oxc-parser 15.0 MB/s, oxc-parser-wasm 13.3 MB/s

**Coverage:** acorn-typescript 3693/4025 (91%), tsv-json 3743/4025 (92%), tsv_wasm-json 3743/4025 (92%), tsv-internal 3743/4025 (92%), tsv_wasm-internal 3743/4025 (92%), oxc-parser 3833/4025 (95%), oxc-parser-wasm 4025/4025 (100%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.7x tsv-internal, tsv_wasm-json 7.6x tsv_wasm-internal

## format/typescript

| Task Name  | ops/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ---------- | ------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier   | 0.11    | 6 | 8.78    | 8.81    | 8.90    | —       | —       | 8.71    | 9.02    | baseline              |
| tsv        | 1.17    | 6 | 0.86    | 0.86    | 0.86    | —       | —       | 0.84    | 0.86    | 10.3x                 |
| tsv_wasm   | 0.87    | 5 | 1.14    | 1.15    | 1.15    | —       | —       | 1.14    | 1.15    | 7.66x                 |
| oxfmt      | 1.25    | 7 | 0.80    | 0.81    | 0.81    | —       | —       | 0.79    | 0.81    | 10.9x                 |
| biome-wasm | 0.27    | 4 | 3.65    | 3.65    | 3.65    | —       | —       | 3.65    | 3.66    | 2.40x                 |

**Files (intersection):** 3701

**Throughput:** prettier 1.5 MB/s, tsv 15.2 MB/s, tsv_wasm 11.3 MB/s, oxfmt 16.2 MB/s, biome-wasm 3.6 MB/s

**Coverage:** prettier 3855/4025 (95%), tsv 3743/4025 (92%), tsv_wasm 3743/4025 (92%), oxfmt 3836/4025 (95%), biome-wasm 4025/4025 (100%)

## parse/css

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 51.54   | 258 | 19.38    | 19.52    | 19.63    | 19.75    | 19.84    | 19.08    | 19.89    | baseline                     |
| tsv-json          | 5.65    | 29  | 176.42   | 178.45   | 179.34   | 179.41   | 179.87   | 173.85   | 180.03   | 0.11x                        |
| tsv_wasm-json     | 4.48    | 18  | 222.99   | 224.04   | 226.00   | 226.36   | 226.69   | 222.56   | 226.78   | 0.09x                        |
| tsv-internal      | 43.25   | 190 | 23.08    | 23.58    | 25.29    | 26.40    | 34.46    | 22.53    | 43.86    | 0.84x                        |
| tsv_wasm-internal | 31.18   | 133 | 32.07    | 32.15    | 32.34    | 32.35    | 32.38    | 31.98    | 32.57    | 0.60x                        |

**Files (intersection):** 159

**Throughput:** svelte/compiler 35.2 MB/s, tsv-json 3.9 MB/s, tsv_wasm-json 3.1 MB/s, tsv-internal 29.5 MB/s, tsv_wasm-internal 21.3 MB/s

**Coverage:** svelte/compiler 163/193 (84%), tsv-json 165/193 (85%), tsv_wasm-json 165/193 (85%), tsv-internal 165/193 (85%), tsv_wasm-internal 165/193 (85%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.6x tsv-internal, tsv_wasm-json 7.0x tsv_wasm-internal

## format/css

| Task Name  | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.37    | 7   | 726.87   | 737.41   | 739.49   | —        | —        | 723.87   | 742.33   | baseline              |
| tsv        | 23.65   | 113 | 42.36    | 42.80    | 43.54    | 44.63    | 50.02    | 40.69    | 51.29    | 17.3x                 |
| tsv_wasm   | 17.59   | 69  | 56.84    | 57.05    | 57.28    | 57.41    | 57.61    | 56.70    | 57.94    | 12.9x                 |
| oxfmt      | 1.32    | 6   | 758.50   | 764.36   | 792.12   | —        | —        | 755.83   | 828.62   | 0.96x                 |
| biome-wasm | 4.82    | 24  | 207.16   | 208.26   | 208.75   | 209.40   | 209.80   | 205.89   | 209.89   | 3.53x                 |

**Files (intersection):** 165

**Throughput:** prettier 0.9 MB/s, tsv 16.4 MB/s, tsv_wasm 12.2 MB/s, oxfmt 0.9 MB/s, biome-wasm 3.3 MB/s

**Coverage:** prettier 192/193 (99%), tsv 165/193 (85%), tsv_wasm 165/193 (85%), oxfmt 192/193 (99%), biome-wasm 193/193 (100%)

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary                    |    Size |  Gzipped | vs tsv | vs tsv (gz) |
| ------------------------- | ------: | -------: | -----: | ----------: |
| tsv_format_wasm           |  2.2 MB | 715.4 KB |   0.8x |        0.8x |
| tsv_parse_wasm            |  1.7 MB | 526.6 KB |   0.6x |        0.6x |
| tsv_wasm                  |  2.9 MB | 911.3 KB |      — |           — |
| biome (wasm)              | 34.4 MB |   8.2 MB |  11.9x |        9.0x |
| oxc-parser (wasm)         |  1.9 MB | 518.7 KB |   0.6x |        0.6x |
| tsv                       |  3.9 MB |   1.6 MB |      — |           — |
| oxc-parser+oxfmt (native) | 10.7 MB |   4.3 MB |   2.8x |        2.6x |
| oxc-parser (native)       |  2.7 MB |   1.0 MB |   0.7x |        0.6x |
| oxfmt (native)            |  8.0 MB |   3.2 MB |   2.1x |        2.0x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark                 | Comparisons                            |
| ------------------------- | -------------------------------------- |
| format svelte (1194f)     | **12.4x** prettier, **13.3x** oxfmt    |
| format typescript (3701f) | **10.3x** prettier, **0.94x** oxfmt    |
| format css (165f)         | **17.3x** prettier, **18.0x** oxfmt    |
| parse svelte (1192f)      | **0.73x** svelte                       |
| parse typescript (3603f)  | **1.08x** svelte, **0.42x** oxc-parser |
| parse css (159f)          | **0.11x** svelte                       |

## Comparisons to tsv_wasm (speedup)

| Benchmark                 | Comparisons                                 |
| ------------------------- | ------------------------------------------- |
| format svelte (1194f)     | **9.06x** prettier, **1.87x** biome-wasm    |
| format typescript (3701f) | **7.66x** prettier, **3.19x** biome-wasm    |
| format css (165f)         | **12.9x** prettier, **3.65x** biome-wasm    |
| parse svelte (1192f)      | **0.69x** svelte                            |
| parse typescript (3603f)  | **1.00x** svelte, **0.44x** oxc-parser-wasm |
| parse css (159f)          | **0.09x** svelte                            |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

1390 unique file+error combinations — Svelte 165, TypeScript 1165, CSS 60.

**Per-benchmark skip counts:**

- parse/typescript: acorn-typescript: 332
- parse/typescript: tsv-json: 282
- parse/typescript: tsv_wasm-json: 282
- parse/typescript: tsv-internal: 282
- parse/typescript: tsv_wasm-internal: 282
- format/typescript: tsv: 282
- format/typescript: tsv_wasm: 282
- parse/typescript: oxc-parser: 192
- format/typescript: oxfmt: 189
- format/typescript: prettier: 170
- parse/svelte: svelte/compiler: 47
- format/svelte: prettier: 39
- format/svelte: oxfmt: 39
- parse/svelte: tsv-json: 38
- parse/svelte: tsv_wasm-json: 38
- parse/svelte: tsv-internal: 38
- parse/svelte: tsv_wasm-internal: 38
- format/svelte: tsv: 38
- format/svelte: tsv_wasm: 38
- parse/css: svelte/compiler: 30
- parse/css: tsv-json: 28
- parse/css: tsv_wasm-json: 28
- parse/css: tsv-internal: 28
- parse/css: tsv_wasm-internal: 28
- format/css: tsv: 28
- format/css: tsv_wasm: 28
- format/svelte: biome-wasm: 2
- format/css: prettier: 1
- format/css: oxfmt: 1

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
