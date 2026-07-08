# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.3.14

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-08T02:14:34.550Z — tsv 0.1.0 (99ac4c69)

**Corpus:** 762 Svelte (1.8 MB), 2395 TypeScript (16.4 MB), 50 CSS (0.3 MB) — 3207 files, 18.5 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (33), ../fuz_code/src (63), ../fuz_css/src (124), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (59), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (144), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 1.38       | 6   | 721.55   | 726.56   | 742.35   | —        | —        | 718.86   | 762.93   | baseline                     |
| tsv-json                   | 6.14       | 31  | 163.12   | 164.05   | 166.26   | 167.61   | 169.23   | 157.58   | 169.76   | 4.44x                        |
| tsv-json-no-locations      | 8.56       | 42  | 116.53   | 118.00   | 120.89   | 121.67   | 125.86   | 113.42   | 128.15   | 6.18x                        |
| tsv_wasm-json              | 5.61       | 27  | 177.87   | 180.47   | 183.66   | 186.19   | 206.51   | 172.26   | 213.63   | 4.06x                        |
| tsv_wasm-json-no-locations | 7.77       | 33  | 128.89   | 131.81   | 159.74   | 163.42   | 166.93   | 125.41   | 167.05   | 5.61x                        |
| tsv-internal               | 49.33      | 230 | 20.08    | 20.82    | 21.06    | 21.16    | 21.35    | 19.82    | 23.33    | 35.6x                        |
| tsv_wasm-internal          | 15.62      | 67  | 63.89    | 65.30    | 66.27    | 94.24    | 99.90    | 58.59    | 103.41   | 11.3x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 2.6 MB/s, tsv-json 11.3 MB/s, tsv-json-no-locations 15.8 MB/s, tsv_wasm-json 10.4 MB/s, tsv_wasm-json-no-locations 14.3 MB/s, tsv-internal 91.0 MB/s, tsv_wasm-internal 28.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.0x tsv-internal, tsv_wasm-json 2.8x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.26       | 7  | 3.88    | 3.99    | 4.00    | —       | —       | 3.80    | 4.02    | baseline              |
| tsv       | 12.69      | 60 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.09    | 49.6x                 |
| tsv_wasm  | 9.21       | 37 | 0.11    | 0.11    | 0.14    | 0.14    | 0.15    | 0.11    | 0.15    | 36.0x                 |
| oxfmt     | 0.20       | 7  | 5.14    | 5.24    | 5.26    | —       | —       | 4.82    | 5.26    | 0.76x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.5 MB/s, tsv 23.4 MB/s, tsv_wasm 17.0 MB/s, oxfmt 0.4 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.17       | 6  | 6076.85  | 6099.57  | 6206.10  | —        | —        | 5991.41  | 6342.34  | baseline                      |
| tsv-json                   | 0.76       | 5  | 1319.45  | 1324.14  | 1324.52  | —        | —        | 1312.85  | 1324.78  | 4.59x                         |
| tsv-json-no-locations      | 1.32       | 6  | 755.79   | 758.65   | 763.47   | —        | —        | 751.13   | 769.90   | 8.01x                         |
| tsv_wasm-json              | 0.70       | 5  | 1428.57  | 1435.26  | 1439.93  | —        | —        | 1416.79  | 1443.05  | 4.23x                         |
| tsv_wasm-json-no-locations | 1.20       | 5  | 831.95   | 832.43   | 838.93   | —        | —        | 830.77   | 845.39   | 7.28x                         |
| tsv-internal               | 8.10       | 32 | 123.60   | 125.16   | 126.86   | 126.98   | 127.05   | 122.73   | 127.09   | 49.0x                         |
| tsv_wasm-internal          | 5.53       | 22 | 180.75   | 182.04   | 185.19   | 185.37   | 185.80   | 179.83   | 185.96   | 33.5x                         |
| oxc-parser                 | 1.08       | 6  | 919.54   | 927.06   | 929.91   | —        | —        | 915.89   | 930.71   | 6.57x                         |

**Files (intersection):** 2392

**Throughput:** acorn-typescript 2.7 MB/s, tsv-json 12.4 MB/s, tsv-json-no-locations 21.6 MB/s, tsv_wasm-json 11.4 MB/s, tsv_wasm-json-no-locations 19.7 MB/s, tsv-internal 132.3 MB/s, tsv_wasm-internal 90.5 MB/s, oxc-parser 17.7 MB/s

**Coverage:** acorn-typescript 2392/2395 (99%), tsv-json 2395/2395 (100%), tsv-json-no-locations 2395/2395 (100%), tsv_wasm-json 2395/2395 (100%), tsv_wasm-json-no-locations 2395/2395 (100%), tsv-internal 2395/2395 (100%), tsv_wasm-internal 2395/2395 (100%), oxc-parser 2393/2395 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.7x tsv-internal, tsv_wasm-json 7.9x tsv_wasm-internal

## format/typescript

| Task Name | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.07       | 7 | 14.86   | 15.15   | 15.29   | —       | —       | 14.71   | 15.39   | baseline              |
| tsv       | 1.87       | 9 | 0.53    | 0.54    | 0.54    | —       | —       | 0.53    | 0.55    | 28.1x                 |
| tsv_wasm  | 1.38       | 6 | 0.73    | 0.73    | 0.74    | —       | —       | 0.72    | 0.75    | 20.6x                 |
| oxfmt     | 0.94       | 5 | 1.06    | 1.07    | 1.07    | —       | —       | 1.06    | 1.07    | 14.1x                 |

**Files (intersection):** 2393

**Throughput:** prettier 1.1 MB/s, tsv 30.6 MB/s, tsv_wasm 22.5 MB/s, oxfmt 15.4 MB/s

**Coverage:** prettier 2395/2395 (100%), tsv 2395/2395 (100%), tsv_wasm 2395/2395 (100%), oxfmt 2393/2395 (99%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 76.86      | 321 | 12.99    | 13.24    | 14.37    | 18.89    | 21.43    | 12.69    | 25.96    | baseline                     |
| tsv-json          | 65.25      | 316 | 15.27    | 15.79    | 16.26    | 16.57    | 19.65    | 14.44    | 23.87    | 0.85x                        |
| tsv_wasm-json     | 58.72      | 283 | 16.91    | 17.36    | 18.09    | 18.36    | 21.36    | 16.41    | 23.18    | 0.76x                        |
| tsv-internal      | 201.84     | 870 | 4.95     | 4.99     | 5.07     | 5.10     | 5.18     | 4.91     | 5.44     | 2.63x                        |
| tsv_wasm-internal | 136.90     | 633 | 7.29     | 7.36     | 7.43     | 7.46     | 7.50     | 7.23     | 11.21    | 1.78x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 22.9 MB/s, tsv-json 19.5 MB/s, tsv_wasm-json 17.5 MB/s, tsv-internal 60.3 MB/s, tsv_wasm-internal 40.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.1x tsv-internal, tsv_wasm-json 2.3x tsv_wasm-internal

## format/css

| Task Name | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| --------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier  | 2.04       | 11  | 484.86   | 497.41   | 500.13   | 503.67   | 506.50   | 478.98   | 507.21   | baseline              |
| tsv       | 107.48     | 533 | 9.28     | 9.39     | 9.48     | 9.53     | 9.65     | 8.99     | 13.89    | 52.6x                 |
| tsv_wasm  | 76.17      | 380 | 13.07    | 13.27    | 13.38    | 13.42    | 13.51    | 12.90    | 16.96    | 37.2x                 |
| oxfmt     | 10.58      | 53  | 94.07    | 96.69    | 99.16    | 100.87   | 104.75   | 85.14    | 105.37   | 5.18x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 32.1 MB/s, tsv_wasm 22.7 MB/s, oxfmt 3.2 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 763.4 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.2 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 843.0 KB | — | — |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.8 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.3x | 3.1x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.6x | 2.4x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **49.6x** prettier, **64.9x** oxfmt |
| format typescript (2393f) | **28.1x** prettier, **1.99x** oxfmt |
| format css (50f) | **52.6x** prettier, **10.2x** oxfmt |
| parse svelte (762f) | **4.44x** svelte |
| parse typescript (2392f) | **4.59x** svelte, **0.70x** oxc-parser |
| parse css (50f) | **0.85x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **36.0x** prettier |
| format typescript (2393f) | **20.6x** prettier |
| format css (50f) | **37.2x** prettier |
| parse svelte (762f) | **4.06x** svelte |
| parse typescript (2392f) | **4.23x** svelte |
| parse css (50f) | **0.76x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

7 unique file+error combinations — Svelte 0, TypeScript 7, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: oxc-parser: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
