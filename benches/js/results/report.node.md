# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-16T11:43:29.034Z — tsv 0.1.0 (135b7b93)

**Corpus:** 763 Svelte (1.9 MB), 2437 TypeScript (16.8 MB), 49 CSS (0.3 MB) — 3249 files, 19.0 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (665), ../fuz_blog/src (37), ../fuz_code/src (66), ../fuz_css/src (146), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (16), ../fuz_ui/src (216), ../fuz_util/src (145), ../mdz/src (71), ../gro/src (156), ../svelte-docinfo/src (99), ../tsv.fuz.dev/src (28), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (297), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.11, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.140.0, oxfmt@0.59.0, @biomejs/wasm-bundler@2.5.4

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.20       | 10  | 452.31   | 458.40   | 464.04   | 466.88   | 469.15   | 449.54   | 469.72   | baseline                     |
| tsv-json                   | 4.65       | 19  | 215.19   | 216.66   | 220.51   | 221.25   | 221.48   | 213.98   | 221.51   | 2.11x                        |
| tsv-json-no-locations      | 7.33       | 32  | 136.25   | 138.65   | 140.12   | 140.43   | 140.67   | 134.36   | 140.78   | 3.33x                        |
| tsv_wasm-json              | 4.13       | 21  | 241.55   | 243.22   | 246.02   | 247.30   | 247.77   | 239.67   | 247.89   | 1.87x                        |
| tsv_wasm-json-no-locations | 6.38       | 30  | 156.58   | 157.97   | 160.52   | 161.07   | 161.41   | 154.63   | 161.52   | 2.90x                        |
| tsv-internal               | 47.87      | 196 | 20.81    | 21.41    | 21.59    | 21.65    | 21.76    | 20.63    | 21.78    | 21.7x                        |
| tsv_wasm-internal          | 26.77      | 134 | 38.08    | 41.06    | 41.95    | 42.20    | 43.26    | 29.81    | 44.79    | 12.2x                        |

**Files (intersection):** 763

**Throughput:** svelte/compiler 4.1 MB/s, tsv-json 8.7 MB/s, tsv-json-no-locations 13.8 MB/s, tsv_wasm-json 7.7 MB/s, tsv_wasm-json-no-locations 12.0 MB/s, tsv-internal 89.9 MB/s, tsv_wasm-internal 50.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.3x tsv-internal, tsv_wasm-json 6.5x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 6  | 4409.03  | 4424.89  | 4449.57  | —        | —        | 4398.62  | 4476.52  | baseline              |
| tsv        | 14.35      | 72 | 69.15    | 71.06    | 71.48    | 71.58    | 71.66    | 68.47    | 71.71    | 63.3x                 |
| tsv_wasm   | 7.93       | 40 | 126.57   | 128.09   | 129.88   | 130.87   | 131.10   | 119.59   | 131.12   | 35.0x                 |
| oxfmt      | 0.23       | 7  | 4378.25  | 4397.74  | 4399.97  | —        | —        | 4277.75  | 4401.39  | 1.01x                 |
| biome-wasm | 1.08       | 6  | 921.26   | 925.93   | 931.60   | —        | —        | 912.93   | 936.17   | 4.78x                 |

**Files (intersection):** 763

**Throughput:** prettier 0.4 MB/s, tsv 27.0 MB/s, tsv_wasm 14.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.32       | 5  | 3.10    | 3.10    | 3.10    | —       | —       | 3.09    | 3.10    | baseline                      |
| tsv-json                   | 0.49       | 4  | 2.05    | 2.06    | 2.06    | —       | —       | 2.05    | 2.07    | 1.51x                         |
| tsv-json-no-locations      | 1.00       | 6  | 1.00    | 1.00    | 1.00    | —       | —       | 1.00    | 1.00    | 3.10x                         |
| tsv_wasm-json              | 0.46       | 5  | 2.19    | 2.20    | 2.20    | —       | —       | 2.18    | 2.20    | 1.41x                         |
| tsv_wasm-json-no-locations | 0.91       | 5  | 1.10    | 1.10    | 1.10    | —       | —       | 1.10    | 1.10    | 2.82x                         |
| tsv-internal               | 7.25       | 26 | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 0.14    | 22.4x                         |
| tsv_wasm-internal          | 5.45       | 22 | 0.18    | 0.18    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 16.9x                         |
| oxc-parser                 | 0.75       | 5  | 1.33    | 1.34    | 1.34    | —       | —       | 1.32    | 1.34    | 2.33x                         |
| oxc-parser-wasm            | 0.74       | 5  | 1.36    | 1.37    | 1.37    | —       | —       | 1.35    | 1.37    | 2.28x                         |

**Files (intersection):** 2434

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.2 MB/s, tsv-json-no-locations 16.8 MB/s, tsv_wasm-json 7.7 MB/s, tsv_wasm-json-no-locations 15.3 MB/s, tsv-internal 121.8 MB/s, tsv_wasm-internal 91.5 MB/s, oxc-parser 12.6 MB/s, oxc-parser-wasm 12.4 MB/s

**Coverage:** acorn-typescript 2434/2437 (99%), tsv-json 2437/2437 (100%), tsv-json-no-locations 2437/2437 (100%), tsv_wasm-json 2437/2437 (100%), tsv_wasm-json-no-locations 2437/2437 (100%), tsv-internal 2437/2437 (100%), tsv_wasm-internal 2437/2437 (100%), oxc-parser 2435/2437 (99%), oxc-parser-wasm 2435/2437 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.9x tsv-internal, tsv_wasm-json 11.9x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.07       | 5 | 13397.85 | 13480.63 | 13560.69 | —        | —        | 13360.49 | 13590.87 | baseline              |
| tsv        | 1.96       | 8 | 510.62   | 511.59   | 518.55   | —        | —        | 509.04   | 527.80   | 26.2x                 |
| tsv_wasm   | 1.40       | 6 | 712.59   | 713.36   | 721.66   | —        | —        | 710.76   | 733.98   | 18.8x                 |
| oxfmt      | 1.18       | 6 | 847.06   | 849.43   | 850.36   | —        | —        | 842.30   | 850.50   | 15.8x                 |
| biome-wasm | 0.22       | 3 | 4664.20  | 6680.54  | 9261.93  | —        | —        | 4599.00  | 10982.86 | 2.89x                 |

**Files (intersection):** 2435

**Throughput:** prettier 1.3 MB/s, tsv 32.9 MB/s, tsv_wasm 23.6 MB/s, oxfmt 19.8 MB/s, biome-wasm 3.6 MB/s

**Coverage:** prettier 2437/2437 (100%), tsv 2437/2437 (100%), tsv_wasm 2437/2437 (100%), oxfmt 2435/2437 (99%), biome-wasm 2437/2437 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 110.63     | 517  | 8.96     | 9.31     | 9.75     | 10.12    | 13.61    | 8.65     | 18.95    | baseline                     |
| tsv-json          | 57.48      | 263  | 17.33    | 17.83    | 17.99    | 19.58    | 20.36    | 16.59    | 20.75    | 0.52x                        |
| tsv_wasm-json     | 54.68      | 233  | 18.18    | 18.67    | 19.46    | 20.19    | 23.13    | 17.92    | 28.34    | 0.49x                        |
| tsv-internal      | 291.88     | 1273 | 3.42     | 3.46     | 3.53     | 3.57     | 3.72     | 3.37     | 5.32     | 2.64x                        |
| tsv_wasm-internal | 203.94     | 803  | 4.90     | 4.96     | 5.02     | 5.05     | 5.10     | 4.85     | 5.27     | 1.84x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 36.2 MB/s, tsv-json 18.8 MB/s, tsv_wasm-json 17.9 MB/s, tsv-internal 95.6 MB/s, tsv_wasm-internal 66.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.1x tsv-internal, tsv_wasm-json 3.7x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.81       | 10  | 548.81   | 561.58   | 564.49   | 565.99   | 567.18   | 539.05   | 567.48   | baseline              |
| tsv        | 142.20     | 654 | 7.01     | 7.10     | 7.21     | 7.24     | 7.29     | 6.93     | 10.84    | 78.5x                 |
| tsv_wasm   | 98.44      | 483 | 10.11    | 10.26    | 10.36    | 10.40    | 10.50    | 9.98     | 13.91    | 54.3x                 |
| oxfmt      | 55.28      | 272 | 18.07    | 18.43    | 18.98    | 19.19    | 20.10    | 16.50    | 20.90    | 30.5x                 |
| biome-wasm | 7.91       | 26  | 126.84   | 138.51   | 193.63   | 202.02   | 208.48   | 124.10   | 209.13   | 4.36x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 46.6 MB/s, tsv_wasm 32.2 MB/s, oxfmt 18.1 MB/s, biome-wasm 2.6 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 794.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 389.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 875.4 KB | — | — |
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
| format svelte (763f) | **63.3x** prettier, **62.4x** oxfmt |
| format typescript (2435f) | **26.2x** prettier, **1.66x** oxfmt |
| format css (49f) | **78.5x** prettier, **2.57x** oxfmt |
| parse svelte (763f) | **2.11x** svelte |
| parse typescript (2434f) | **1.51x** svelte, **0.65x** oxc-parser |
| parse css (49f) | **0.52x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (763f) | **35.0x** prettier, **7.32x** biome-wasm |
| format typescript (2435f) | **18.8x** prettier, **6.51x** biome-wasm |
| format css (49f) | **54.3x** prettier, **12.4x** biome-wasm |
| parse svelte (763f) | **1.87x** svelte |
| parse typescript (2434f) | **1.41x** svelte, **0.62x** oxc-parser-wasm |
| parse css (49f) | **0.49x** svelte |

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
