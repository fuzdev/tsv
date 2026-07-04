# tsv conformance benchmark results (parse)

**Runtime:** deno

**Corpus kind:** conformance — full fixtures-included corpus, parse groups only; the headline is the per-tool Coverage lines (parse success over the full set), with throughput measured on the all-tools-pass intersection

**Date:** 2026-07-04T22:56:52.134Z — tsv 0.1.0 (34a14e63)

**Corpus:** 50 Svelte (0.1 MB), 50 TypeScript (0.2 MB), 50 CSS (0.1 MB) — 150 files, 0.5 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (68), ../fuz_css/src (124), ../fuz_docs/src (64), ../fuz_gitops/src (203), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../gro/src (187), ../svelte-docinfo/src (297), ../tsv.fuz.dev/src (26), ../kit/packages/kit/src (380), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65), ../prettier-plugin-svelte/test (325), ../prettier/tests/format/typescript (789), ../prettier/tests/format/js (1103), ../prettier/tests/format/css (228), ../prettier/tests/format/html (124), ../svelte/packages/svelte/tests (4543), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (42113)

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 25.02   | 23  | 40.13    | 41.24    | 44.22    | 48.18    | 55.77    | 36.90    | 57.90    | baseline                     |
| tsv-json          | 63.20   | 63  | 15.68    | 16.18    | 16.65    | 16.82    | 17.18    | 15.12    | 17.42    | 2.53x                        |
| tsv_wasm-json     | 50.83   | 51  | 19.63    | 19.83    | 20.13    | 20.46    | 20.63    | 19.03    | 20.67    | 2.03x                        |
| tsv-internal      | 559.04  | 519 | 1.79     | 1.83     | 1.89     | 1.98     | 2.51     | 1.59     | 11.72    | 22.3x                        |
| tsv_wasm-internal | 380.50  | 370 | 2.62     | 2.68     | 2.76     | 2.82     | 3.03     | 2.48     | 3.60     | 15.2x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 3.3 MB/s, tsv-json 8.2 MB/s, tsv_wasm-json 6.6 MB/s, tsv-internal 72.7 MB/s, tsv_wasm-internal 49.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.8x tsv-internal, tsv_wasm-json 7.5x tsv_wasm-internal

## parse/typescript

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript  | 22.02   | 21  | 45.13    | 47.05    | 48.69    | 50.06    | 58.05    | 42.79    | 60.15    | baseline                      |
| tsv-json          | 35.97   | 33  | 27.65    | 28.44    | 29.24    | 31.59    | 32.44    | 26.85    | 32.51    | 1.63x                         |
| tsv_wasm-json     | 30.43   | 31  | 32.74    | 33.37    | 33.77    | 33.94    | 34.25    | 31.83    | 34.36    | 1.38x                         |
| tsv-internal      | 433.04  | 411 | 2.31     | 2.36     | 2.43     | 2.49     | 3.46     | 2.15     | 9.78     | 19.7x                         |
| tsv_wasm-internal | 284.92  | 283 | 3.51     | 3.55     | 3.59     | 3.63     | 3.69     | 3.37     | 4.08     | 12.9x                         |
| oxc-parser        | 64.79   | 40  | 15.63    | 24.59    | 25.73    | 25.95    | 27.34    | 14.62    | 27.55    | 2.94x                         |
| oxc-parser-wasm   | 47.61   | 46  | 20.74    | 21.47    | 22.97    | 23.36    | 26.19    | 19.81    | 27.10    | 2.16x                         |

**Files (intersection):** 50

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.9 MB/s, tsv_wasm-json 7.5 MB/s, tsv-internal 106.7 MB/s, tsv_wasm-internal 70.2 MB/s, oxc-parser 16.0 MB/s, oxc-parser-wasm 11.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 12.0x tsv-internal, tsv_wasm-json 9.4x tsv_wasm-internal

## parse/css

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 284.35  | 217 | 3.52     | 3.77     | 4.11     | 4.17     | 4.57     | 3.35     | 7.32     | baseline                     |
| tsv-json          | 159.89  | 141 | 6.24     | 6.48     | 7.02     | 7.93     | 9.53     | 5.85     | 11.79    | 0.56x                        |
| tsv_wasm-json     | 122.32  | 116 | 8.18     | 8.30     | 8.52     | 8.79     | 9.62     | 7.86     | 10.79    | 0.43x                        |
| tsv-internal      | 406.55  | 399 | 2.46     | 2.49     | 2.54     | 2.56     | 2.69     | 2.36     | 2.93     | 1.43x                        |
| tsv_wasm-internal | 255.26  | 251 | 3.92     | 3.96     | 4.00     | 4.04     | 4.20     | 3.79     | 4.28     | 0.90x                        |

**Files (intersection):** 48

**Throughput:** svelte/compiler 36.4 MB/s, tsv-json 20.5 MB/s, tsv_wasm-json 15.7 MB/s, tsv-internal 52.1 MB/s, tsv_wasm-internal 32.7 MB/s

**Coverage:** svelte/compiler 48/50 (96%), tsv-json 48/50 (96%), tsv_wasm-json 48/50 (96%), tsv-internal 48/50 (96%), tsv_wasm-internal 48/50 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.5x tsv-internal, tsv_wasm-json 2.1x tsv_wasm-internal

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 784.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 401.4 KB | 0.4x | 0.5x |
| tsv_wasm | 2.5 MB | 866.1 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 13.6x | 9.5x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.2x | 3.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 683.6 KB | 0.5x | 0.5x |
| tsv (napi) | 3.8 MB | 1.6 MB | 1.2x | 1.1x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.4x | 2.3x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| parse svelte (50f) | **2.53x** svelte |
| parse typescript (50f) | **1.63x** svelte, **0.56x** oxc-parser |
| parse css (48f) | **0.56x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| parse svelte (50f) | **2.03x** svelte |
| parse typescript (50f) | **1.38x** svelte, **0.64x** oxc-parser-wasm |
| parse css (48f) | **0.43x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally._

## Skipped Files

4 unique file+error combinations — Svelte 0, TypeScript 0, CSS 4.

**Per-benchmark skip counts:**
- parse/css: svelte/compiler: 2
- parse/css: tsv-json: 2
- parse/css: tsv_wasm-json: 2
- parse/css: tsv-internal: 2
- parse/css: tsv_wasm-internal: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
