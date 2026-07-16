# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-16T00:15:41.741Z — tsv 0.1.0 (eca5466f)

**Corpus:** 763 Svelte (1.9 MB), 2437 TypeScript (16.8 MB), 49 CSS (0.3 MB) — 3249 files, 19.0 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.59.0, @biomejs/wasm-bundler@2.5.4

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.24       | 11  | 446.24   | 451.11   | 457.98   | 464.63   | 470.68   | 442.69   | 472.19   | baseline                     |
| tsv-json                   | 4.65       | 24  | 214.56   | 217.84   | 218.92   | 219.06   | 219.13   | 212.13   | 219.15   | 2.08x                        |
| tsv-json-no-locations      | 7.35       | 37  | 135.97   | 137.93   | 138.87   | 139.08   | 139.09   | 133.62   | 139.10   | 3.29x                        |
| tsv_wasm-json              | 4.18       | 21  | 238.07   | 243.54   | 244.28   | 244.69   | 244.94   | 234.17   | 245.00   | 1.87x                        |
| tsv_wasm-json-no-locations | 6.40       | 33  | 155.34   | 158.32   | 159.51   | 159.78   | 160.13   | 153.13   | 160.15   | 2.87x                        |
| tsv-internal               | 47.38      | 225 | 20.94    | 21.52    | 21.68    | 21.80    | 21.94    | 20.71    | 21.99    | 21.2x                        |
| tsv_wasm-internal          | 27.08      | 136 | 37.76    | 40.59    | 41.54    | 42.05    | 42.60    | 28.61    | 42.78    | 12.1x                        |

**Files (intersection):** 763

**Throughput:** svelte/compiler 4.2 MB/s, tsv-json 8.7 MB/s, tsv-json-no-locations 13.8 MB/s, tsv_wasm-json 7.9 MB/s, tsv_wasm-json-no-locations 12.0 MB/s, tsv-internal 89.0 MB/s, tsv_wasm-internal 50.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.2x tsv-internal, tsv_wasm-json 6.5x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22       | 6  | 4443.22  | 4549.81  | 4633.23  | —        | —        | 4414.42  | 4722.40  | baseline              |
| tsv        | 14.57      | 52 | 68.56    | 70.73    | 71.07    | 71.16    | 71.29    | 68.24    | 71.48    | 65.2x                 |
| tsv_wasm   | 7.97       | 40 | 125.11   | 126.81   | 131.01   | 133.43   | 133.88   | 119.80   | 134.10   | 35.6x                 |
| oxfmt      | 0.23       | 7  | 4381.49  | 4423.11  | 4436.92  | —        | —        | 4285.70  | 4442.08  | 1.02x                 |
| biome-wasm | 1.09       | 6  | 919.13   | 925.86   | 929.44   | —        | —        | 907.22   | 931.72   | 4.86x                 |

**Files (intersection):** 763

**Throughput:** prettier 0.4 MB/s, tsv 27.4 MB/s, tsv_wasm 15.0 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.32       | 5  | 3.13    | 3.13    | 3.14    | —       | —       | 3.12    | 3.14    | baseline                      |
| tsv-json                   | 0.50       | 5  | 2.01    | 2.01    | 2.02    | —       | —       | 2.01    | 2.02    | 1.55x                         |
| tsv-json-no-locations      | 1.01       | 6  | 0.99    | 0.99    | 0.99    | —       | —       | 0.99    | 0.99    | 3.16x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.14    | 2.14    | 2.14    | —       | —       | 2.13    | 2.14    | 1.46x                         |
| tsv_wasm-json-no-locations | 0.92       | 5  | 1.08    | 1.08    | 1.08    | —       | —       | 1.08    | 1.08    | 2.89x                         |
| tsv-internal               | 7.23       | 32 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 22.6x                         |
| tsv_wasm-internal          | 5.51       | 22 | 0.18    | 0.18    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 17.2x                         |
| oxc-parser                 | 0.77       | 5  | 1.30    | 1.32    | 1.32    | —       | —       | 1.29    | 1.32    | 2.40x                         |
| oxc-parser-wasm            | 0.75       | 5  | 1.34    | 1.35    | 1.35    | —       | —       | 1.33    | 1.35    | 2.33x                         |

**Files (intersection):** 2434

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.3 MB/s, tsv-json-no-locations 16.9 MB/s, tsv_wasm-json 7.9 MB/s, tsv_wasm-json-no-locations 15.5 MB/s, tsv-internal 121.5 MB/s, tsv_wasm-internal 92.6 MB/s, oxc-parser 12.9 MB/s, oxc-parser-wasm 12.5 MB/s

**Coverage:** acorn-typescript 2434/2437 (99%), tsv-json 2437/2437 (100%), tsv-json-no-locations 2437/2437 (100%), tsv_wasm-json 2437/2437 (100%), tsv_wasm-json-no-locations 2437/2437 (100%), tsv-internal 2437/2437 (100%), tsv_wasm-internal 2437/2437 (100%), oxc-parser 2435/2437 (99%), oxc-parser-wasm 2435/2437 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.6x tsv-internal, tsv_wasm-json 11.8x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.07       | 7 | 13688.70 | 13737.48 | 13794.21 | —        | —        | 13620.41 | 13821.42 | baseline              |
| tsv        | 1.96       | 8 | 509.18   | 511.31   | 518.12   | —        | —        | 507.90   | 521.49   | 26.9x                 |
| tsv_wasm   | 1.40       | 6 | 713.44   | 715.42   | 723.57   | —        | —        | 712.21   | 735.27   | 19.2x                 |
| oxfmt      | 1.18       | 6 | 846.47   | 847.85   | 849.89   | —        | —        | 842.13   | 851.80   | 16.2x                 |
| biome-wasm | 0.22       | 3 | 4573.99  | 6561.24  | 9142.30  | —        | —        | 4509.55  | 10863.00 | 3.02x                 |

**Files (intersection):** 2435

**Throughput:** prettier 1.2 MB/s, tsv 33.0 MB/s, tsv_wasm 23.6 MB/s, oxfmt 19.9 MB/s, biome-wasm 3.7 MB/s

**Coverage:** prettier 2437/2437 (100%), tsv 2437/2437 (100%), tsv_wasm 2437/2437 (100%), oxfmt 2435/2437 (99%), biome-wasm 2437/2437 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 106.98     | 508  | 9.29     | 9.63     | 10.11    | 10.41    | 14.67    | 8.74     | 18.67    | baseline                     |
| tsv-json          | 57.63      | 264  | 17.26    | 17.81    | 18.13    | 19.44    | 20.34    | 16.52    | 20.69    | 0.54x                        |
| tsv_wasm-json     | 55.16      | 250  | 18.05    | 18.46    | 19.30    | 19.99    | 23.78    | 17.67    | 25.38    | 0.52x                        |
| tsv-internal      | 289.85     | 1277 | 3.44     | 3.48     | 3.55     | 3.57     | 3.61     | 3.38     | 3.88     | 2.71x                        |
| tsv_wasm-internal | 205.02     | 870  | 4.87     | 4.93     | 5.00     | 5.04     | 5.10     | 4.79     | 5.38     | 1.92x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 35.0 MB/s, tsv-json 18.9 MB/s, tsv_wasm-json 18.1 MB/s, tsv-internal 95.0 MB/s, tsv_wasm-internal 67.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.0x tsv-internal, tsv_wasm-json 3.7x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.82       | 10  | 548.30   | 553.20   | 562.00   | 564.49   | 566.49   | 532.09   | 566.98   | baseline              |
| tsv        | 143.37     | 561 | 6.97     | 7.09     | 7.24     | 7.30     | 7.45     | 6.91     | 10.35    | 78.8x                 |
| tsv_wasm   | 99.87      | 426 | 9.99     | 10.15    | 10.29    | 10.37    | 10.56    | 9.89     | 13.67    | 54.9x                 |
| oxfmt      | 54.15      | 268 | 18.45    | 18.88    | 19.38    | 19.69    | 20.40    | 16.52    | 22.19    | 29.8x                 |
| biome-wasm | 7.94       | 26  | 126.10   | 137.89   | 194.02   | 197.87   | 207.45   | 123.91   | 208.56   | 4.37x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 47.0 MB/s, tsv_wasm 32.7 MB/s, oxfmt 17.7 MB/s, biome-wasm 2.6 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 794.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 389.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 875.3 KB | — | — |
| biome (wasm) | 38.6 MB | 9.3 MB | 15.5x | 10.6x |
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
| format svelte (763f) | **65.2x** prettier, **63.7x** oxfmt |
| format typescript (2435f) | **26.9x** prettier, **1.66x** oxfmt |
| format css (49f) | **78.8x** prettier, **2.65x** oxfmt |
| parse svelte (763f) | **2.08x** svelte |
| parse typescript (2434f) | **1.55x** svelte, **0.65x** oxc-parser |
| parse css (49f) | **0.54x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **35.6x** prettier, **7.33x** biome-wasm |
| format typescript (2435f) | **19.2x** prettier, **6.37x** biome-wasm |
| format css (49f) | **54.9x** prettier, **12.6x** biome-wasm |
| parse svelte (763f) | **1.87x** svelte |
| parse typescript (2434f) | **1.46x** svelte, **0.63x** oxc-parser-wasm |
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
