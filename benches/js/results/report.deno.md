# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.8.3

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-16T00:07:00.449Z — tsv 0.1.0 (eca5466f)

**Corpus:** 763 Svelte (1.9 MB), 2437 TypeScript (16.8 MB), 49 CSS (0.3 MB) — 3249 files, 19.0 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.59.0, @biomejs/wasm-bundler@2.5.4

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.29       | 11  | 436.94   | 439.64   | 441.31   | 445.25   | 448.98   | 428.76   | 449.92   | baseline                     |
| tsv-json                   | 4.98       | 25  | 201.71   | 203.45   | 203.98   | 204.13   | 204.18   | 196.34   | 204.20   | 2.18x                        |
| tsv-json-no-locations      | 7.77       | 39  | 129.39   | 130.36   | 130.81   | 130.84   | 131.14   | 124.97   | 131.26   | 3.40x                        |
| tsv_wasm-json              | 4.10       | 21  | 244.03   | 246.53   | 247.15   | 247.62   | 248.28   | 238.95   | 248.44   | 1.79x                        |
| tsv_wasm-json-no-locations | 6.23       | 30  | 159.93   | 162.95   | 163.88   | 164.19   | 179.61   | 157.74   | 186.22   | 2.73x                        |
| tsv-internal               | 46.19      | 231 | 21.56    | 21.96    | 22.11    | 22.21    | 22.37    | 21.11    | 22.57    | 20.2x                        |
| tsv_wasm-internal          | 30.24      | 148 | 32.98    | 33.36    | 33.83    | 33.98    | 36.67    | 32.51    | 45.91    | 13.2x                        |

**Files (intersection):** 763

**Throughput:** svelte/compiler 4.3 MB/s, tsv-json 9.4 MB/s, tsv-json-no-locations 14.6 MB/s, tsv_wasm-json 7.7 MB/s, tsv_wasm-json-no-locations 11.7 MB/s, tsv-internal 86.7 MB/s, tsv_wasm-internal 56.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.3x tsv-internal, tsv_wasm-json 7.4x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4299.03  | 4324.04  | 4395.28  | —        | —        | 4156.09  | 4478.21  | baseline              |
| tsv        | 14.13      | 70 | 70.30    | 71.79    | 72.52    | 72.68    | 73.14    | 69.47    | 73.98    | 60.4x                 |
| tsv_wasm   | 9.04       | 39 | 110.42   | 112.39   | 114.22   | 114.63   | 114.90   | 109.35   | 115.00   | 38.7x                 |
| oxfmt      | 0.24       | 4  | 4182.98  | 4222.43  | 4235.09  | —        | —        | 4176.21  | 4243.53  | 1.02x                 |
| biome-wasm | 1.43       | 7  | 698.18   | 699.19   | 706.45   | —        | —        | 695.28   | 722.77   | 6.13x                 |

**Files (intersection):** 763

**Throughput:** prettier 0.4 MB/s, tsv 26.5 MB/s, tsv_wasm 17.0 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.7 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.35       | 5  | 2.83    | 2.83    | 2.83    | —       | —       | 2.82    | 2.84    | baseline                      |
| tsv-json                   | 0.55       | 5  | 1.82    | 1.84    | 1.85    | —       | —       | 1.80    | 1.85    | 1.55x                         |
| tsv-json-no-locations      | 1.14       | 5  | 0.88    | 0.88    | 0.88    | —       | —       | 0.88    | 0.88    | 3.23x                         |
| tsv_wasm-json              | 0.48       | 4  | 2.10    | 2.10    | 2.11    | —       | —       | 2.10    | 2.11    | 1.35x                         |
| tsv_wasm-json-no-locations | 0.93       | 5  | 1.07    | 1.07    | 1.07    | —       | —       | 1.07    | 1.07    | 2.64x                         |
| tsv-internal               | 7.58       | 29 | 0.13    | 0.13    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 21.5x                         |
| tsv_wasm-internal          | 4.82       | 21 | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 0.21    | 13.6x                         |
| oxc-parser                 | 0.86       | 5  | 1.17    | 1.17    | 1.18    | —       | —       | 1.14    | 1.18    | 2.44x                         |
| oxc-parser-wasm            | 0.76       | 4  | 1.30    | 1.31    | 1.31    | —       | —       | 1.30    | 1.31    | 2.16x                         |

**Files (intersection):** 2434

**Throughput:** acorn-typescript 5.9 MB/s, tsv-json 9.2 MB/s, tsv-json-no-locations 19.2 MB/s, tsv_wasm-json 8.0 MB/s, tsv_wasm-json-no-locations 15.7 MB/s, tsv-internal 127.4 MB/s, tsv_wasm-internal 80.9 MB/s, oxc-parser 14.5 MB/s, oxc-parser-wasm 12.8 MB/s

**Coverage:** acorn-typescript 2434/2437 (99%), tsv-json 2437/2437 (100%), tsv-json-no-locations 2437/2437 (100%), tsv_wasm-json 2437/2437 (100%), tsv_wasm-json-no-locations 2437/2437 (100%), tsv-internal 2437/2437 (100%), tsv_wasm-internal 2437/2437 (100%), oxc-parser 2435/2437 (99%), oxc-parser-wasm 2435/2437 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.8x tsv-internal, tsv_wasm-json 10.1x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 7 | 12193.80 | 12329.09 | 12454.65 | —        | —        | 12073.43 | 12465.88 | baseline              |
| tsv        | 2.00       | 7 | 501.14   | 502.20   | 510.66   | —        | —        | 500.45   | 518.01   | 24.4x                 |
| tsv_wasm   | 1.27       | 6 | 787.78   | 788.17   | 794.95   | —        | —        | 785.83   | 804.68   | 15.5x                 |
| oxfmt      | 1.20       | 6 | 834.09   | 837.47   | 840.28   | —        | —        | 829.72   | 842.47   | 14.7x                 |
| biome-wasm | 0.24       | 4 | 4134.32  | 4139.48  | 4150.17  | —        | —        | 4130.25  | 4157.30  | 2.96x                 |

**Files (intersection):** 2435

**Throughput:** prettier 1.4 MB/s, tsv 33.6 MB/s, tsv_wasm 21.3 MB/s, oxfmt 20.1 MB/s, biome-wasm 4.1 MB/s

**Coverage:** prettier 2437/2437 (100%), tsv 2437/2437 (100%), tsv_wasm 2437/2437 (100%), oxfmt 2435/2437 (99%), biome-wasm 2437/2437 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 98.93      | 485  | 10.06    | 10.31    | 10.59    | 10.82    | 11.42    | 9.64     | 17.14    | baseline                     |
| tsv-json          | 66.35      | 298  | 15.01    | 15.34    | 15.74    | 16.69    | 17.38    | 14.75    | 25.59    | 0.67x                        |
| tsv_wasm-json     | 51.91      | 252  | 19.14    | 19.59    | 19.82    | 20.37    | 21.07    | 18.75    | 21.61    | 0.52x                        |
| tsv-internal      | 302.22     | 1267 | 3.30     | 3.34     | 3.39     | 3.42     | 3.45     | 3.27     | 3.67     | 3.05x                        |
| tsv_wasm-internal | 181.94     | 798  | 5.48     | 5.54     | 5.62     | 5.64     | 5.72     | 5.45     | 9.49     | 1.84x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 32.4 MB/s, tsv-json 21.7 MB/s, tsv_wasm-json 17.0 MB/s, tsv-internal 99.0 MB/s, tsv_wasm-internal 59.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.6x tsv-internal, tsv_wasm-json 3.5x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.96       | 9   | 510.22   | 515.17   | 522.18   | —        | —        | 507.04   | 525.87   | baseline              |
| tsv        | 148.55     | 612 | 6.72     | 6.84     | 6.97     | 7.04     | 7.21     | 6.66     | 7.56     | 76.0x                 |
| tsv_wasm   | 87.91      | 423 | 11.32    | 11.51    | 11.62    | 11.67    | 11.91    | 11.22    | 12.10    | 45.0x                 |
| oxfmt      | 55.36      | 274 | 18.03    | 18.40    | 18.88    | 19.13    | 19.71    | 16.44    | 20.30    | 28.3x                 |
| biome-wasm | 10.30      | 51  | 96.67    | 98.16    | 99.03    | 99.23    | 99.43    | 95.81    | 99.47    | 5.27x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 48.7 MB/s, tsv_wasm 28.8 MB/s, oxfmt 18.1 MB/s, biome-wasm 3.4 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 794.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 389.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 875.3 KB | — | — |
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
| format svelte (763f) | **60.4x** prettier, **59.2x** oxfmt |
| format typescript (2435f) | **24.4x** prettier, **1.67x** oxfmt |
| format css (49f) | **76.0x** prettier, **2.68x** oxfmt |
| parse svelte (763f) | **2.18x** svelte |
| parse typescript (2434f) | **1.55x** svelte, **0.64x** oxc-parser |
| parse css (49f) | **0.67x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **38.7x** prettier, **6.31x** biome-wasm |
| format typescript (2435f) | **15.5x** prettier, **5.25x** biome-wasm |
| format css (49f) | **45.0x** prettier, **8.53x** biome-wasm |
| parse svelte (763f) | **1.79x** svelte |
| parse typescript (2434f) | **1.35x** svelte, **0.62x** oxc-parser-wasm |
| parse css (49f) | **0.52x** svelte |

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
