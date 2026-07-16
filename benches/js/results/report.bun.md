# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.3.14

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-16T11:51:31.124Z — tsv 0.1.0 (135b7b93)

**Corpus:** 763 Svelte (1.9 MB), 2437 TypeScript (16.8 MB), 49 CSS (0.3 MB) — 3249 files, 19.0 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.59.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 1.35       | 6   | 740.44   | 746.11   | 758.43   | —        | —        | 734.42   | 773.64   | baseline                     |
| tsv-json                   | 6.01       | 31  | 166.86   | 168.47   | 170.99   | 171.59   | 172.84   | 161.15   | 173.38   | 4.44x                        |
| tsv-json-no-locations      | 8.52       | 43  | 117.52   | 119.15   | 120.99   | 121.31   | 123.13   | 113.88   | 123.54   | 6.30x                        |
| tsv_wasm-json              | 5.56       | 26  | 180.09   | 181.98   | 186.42   | 197.94   | 205.41   | 174.79   | 206.31   | 4.12x                        |
| tsv_wasm-json-no-locations | 7.66       | 34  | 130.98   | 133.58   | 144.32   | 161.55   | 164.51   | 126.36   | 165.69   | 5.66x                        |
| tsv-internal               | 51.07      | 222 | 19.44    | 20.19    | 20.38    | 20.48    | 20.80    | 19.21    | 20.86    | 37.8x                        |
| tsv_wasm-internal          | 15.85      | 70  | 62.70    | 64.67    | 65.69    | 66.38    | 96.14    | 58.39    | 98.50    | 11.7x                        |

**Files (intersection):** 763

**Throughput:** svelte/compiler 2.5 MB/s, tsv-json 11.3 MB/s, tsv-json-no-locations 16.0 MB/s, tsv_wasm-json 10.4 MB/s, tsv_wasm-json-no-locations 14.4 MB/s, tsv-internal 95.9 MB/s, tsv_wasm-internal 29.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.5x tsv-internal, tsv_wasm-json 2.8x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.25       | 7  | 3.95    | 3.99    | 4.08    | —       | —       | 3.86    | 4.16    | baseline              |
| tsv       | 14.12      | 56 | 0.07    | 0.07    | 0.07    | 0.07    | 0.08    | 0.07    | 0.08    | 55.9x                 |
| tsv_wasm  | 7.19       | 36 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.15    | 28.5x                 |
| oxfmt     | 0.19       | 7  | 5.26    | 5.33    | 5.40    | —       | —       | 5.01    | 5.49    | 0.75x                 |

**Files (intersection):** 763

**Throughput:** prettier 0.5 MB/s, tsv 26.5 MB/s, tsv_wasm 13.5 MB/s, oxfmt 0.4 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.15       | 6  | 6586.62  | 6597.41  | 6705.96  | —        | —        | 6554.21  | 6864.56  | baseline                      |
| tsv-json                   | 0.74       | 5  | 1351.18  | 1354.83  | 1355.05  | —        | —        | 1347.35  | 1355.19  | 4.87x                         |
| tsv-json-no-locations      | 1.28       | 6  | 779.69   | 781.04   | 786.80   | —        | —        | 776.25   | 793.99   | 8.45x                         |
| tsv_wasm-json              | 0.68       | 5  | 1459.71  | 1463.86  | 1467.08  | —        | —        | 1455.93  | 1469.22  | 4.50x                         |
| tsv_wasm-json-no-locations | 1.17       | 6  | 857.16   | 857.71   | 858.61   | —        | —        | 855.48   | 859.47   | 7.68x                         |
| tsv-internal               | 8.30       | 32 | 120.62   | 121.62   | 122.91   | 122.97   | 123.68   | 119.55   | 123.76   | 54.6x                         |
| tsv_wasm-internal          | 5.70       | 28 | 174.94   | 176.51   | 179.23   | 179.38   | 179.56   | 173.30   | 179.61   | 37.5x                         |
| oxc-parser                 | 1.05       | 6  | 953.81   | 958.01   | 960.99   | —        | —        | 944.50   | 963.25   | 6.90x                         |

**Files (intersection):** 2434

**Throughput:** acorn-typescript 2.6 MB/s, tsv-json 12.4 MB/s, tsv-json-no-locations 21.6 MB/s, tsv_wasm-json 11.5 MB/s, tsv_wasm-json-no-locations 19.6 MB/s, tsv-internal 139.4 MB/s, tsv_wasm-internal 95.7 MB/s, oxc-parser 17.6 MB/s

**Coverage:** acorn-typescript 2434/2437 (99%), tsv-json 2437/2437 (100%), tsv-json-no-locations 2437/2437 (100%), tsv_wasm-json 2437/2437 (100%), tsv_wasm-json-no-locations 2437/2437 (100%), tsv-internal 2437/2437 (100%), tsv_wasm-internal 2437/2437 (100%), oxc-parser 2435/2437 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.2x tsv-internal, tsv_wasm-json 8.3x tsv_wasm-internal

## format/typescript

| Task Name | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.06       | 7 | 15.99   | 16.13   | 16.27   | —       | —       | 15.61   | 16.35   | baseline              |
| tsv       | 1.94       | 7 | 0.52    | 0.52    | 0.52    | —       | —       | 0.51    | 0.53    | 31.0x                 |
| tsv_wasm  | 1.44       | 7 | 0.69    | 0.70    | 0.70    | —       | —       | 0.69    | 0.72    | 23.0x                 |
| oxfmt     | 0.99       | 4 | 1.01    | 1.01    | 1.01    | —       | —       | 1.00    | 1.01    | 15.9x                 |

**Files (intersection):** 2435

**Throughput:** prettier 1.1 MB/s, tsv 32.6 MB/s, tsv_wasm 24.2 MB/s, oxfmt 16.7 MB/s

**Coverage:** prettier 2437/2437 (100%), tsv 2437/2437 (100%), tsv_wasm 2437/2437 (100%), oxfmt 2435/2437 (99%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 68.18      | 291  | 14.63    | 14.90    | 16.82    | 21.20    | 24.42    | 14.34    | 26.23    | baseline                     |
| tsv-json          | 73.63      | 355  | 13.54    | 14.01    | 14.44    | 14.83    | 17.81    | 12.67    | 29.01    | 1.08x                        |
| tsv_wasm-json     | 68.46      | 334  | 14.47    | 14.91    | 15.60    | 15.90    | 17.99    | 13.86    | 19.11    | 1.00x                        |
| tsv-internal      | 325.20     | 1573 | 3.06     | 3.11     | 3.15     | 3.18     | 3.24     | 3.02     | 3.31     | 4.77x                        |
| tsv_wasm-internal | 217.84     | 914  | 4.58     | 4.64     | 4.70     | 4.72     | 4.75     | 4.55     | 4.96     | 3.19x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 22.3 MB/s, tsv-json 24.1 MB/s, tsv_wasm-json 22.4 MB/s, tsv-internal 106.5 MB/s, tsv_wasm-internal 71.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.4x tsv-internal, tsv_wasm-json 3.2x tsv_wasm-internal

## format/css

| Task Name | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 1.91       | 9   | 524.31   | 528.55   | 531.43   | —        | —        | 507.06   | 547.11   | baseline              |
| tsv       | 147.46     | 718 | 6.76     | 6.86     | 6.97     | 7.01     | 7.19     | 6.49     | 11.54    | 77.1x                 |
| tsv_wasm  | 101.40     | 504 | 9.83     | 9.95     | 10.07    | 10.12    | 10.25    | 9.67     | 10.41    | 53.0x                 |
| oxfmt     | 47.44      | 238 | 21.09    | 22.74    | 23.64    | 24.24    | 26.33    | 17.84    | 27.33    | 24.8x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 48.3 MB/s, tsv_wasm 33.2 MB/s, oxfmt 15.5 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 794.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 389.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 875.4 KB | — | — |
| oxc-parser (wasm) | 1.5 MB | 495.2 KB | 0.6x | 0.6x |
| tsv (ffi) | 3.4 MB | 1.5 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 705.0 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.3 MB | 4.5 MB | 3.2x | 3.0x |
| oxc-parser (napi) | 2.4 MB | 954.8 KB | 0.7x | 0.6x |
| oxfmt (napi) | 8.9 MB | 3.6 MB | 2.5x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **55.9x** prettier, **74.2x** oxfmt |
| format typescript (2435f) | **31.0x** prettier, **1.95x** oxfmt |
| format css (49f) | **77.1x** prettier, **3.11x** oxfmt |
| parse svelte (763f) | **4.44x** svelte |
| parse typescript (2434f) | **4.87x** svelte, **0.71x** oxc-parser |
| parse css (49f) | **1.08x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **28.5x** prettier |
| format typescript (2435f) | **23.0x** prettier |
| format css (49f) | **53.0x** prettier |
| parse svelte (763f) | **4.12x** svelte |
| parse typescript (2434f) | **4.50x** svelte |
| parse css (49f) | **1.00x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

7 unique file+error combinations — Svelte 0, TypeScript 7, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: oxc-parser: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
