# tsv conformance benchmark results (parse)

**Runtime:** node

**Corpus kind:** conformance — full fixtures-included corpus, parse groups only; the headline is the per-tool Coverage lines (parse success over the full set), with throughput measured on the all-tools-pass intersection

**Date:** 2026-07-04T22:58:25.136Z — tsv 0.1.0 (34a14e63)

**Corpus:** 50 Svelte (0.1 MB), 50 TypeScript (0.2 MB), 50 CSS (0.1 MB) — 150 files, 0.5 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (68), ../fuz_css/src (124), ../fuz_docs/src (64), ../fuz_gitops/src (203), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../gro/src (187), ../svelte-docinfo/src (297), ../tsv.fuz.dev/src (26), ../kit/packages/kit/src (381), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65), ../prettier-plugin-svelte/test (325), ../prettier/tests/format/typescript (789), ../prettier/tests/format/js (1103), ../prettier/tests/format/css (228), ../prettier/tests/format/html (124), ../svelte/packages/svelte/tests (4543), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (42113)

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 23.48   | 23  | 42.37    | 45.10    | 45.95    | 46.79    | 52.09    | 39.00    | 53.63    | baseline                     |
| tsv-json          | 57.08   | 56  | 17.48    | 17.76    | 18.23    | 18.62    | 18.94    | 16.82    | 19.23    | 2.43x                        |
| tsv_wasm-json     | 50.16   | 49  | 19.89    | 20.36    | 20.80    | 21.01    | 21.74    | 19.13    | 22.40    | 2.14x                        |
| tsv-internal      | 640.60  | 623 | 1.56     | 1.60     | 1.64     | 1.71     | 1.86     | 1.45     | 2.58     | 27.3x                        |
| tsv_wasm-internal | 425.18  | 416 | 2.35     | 2.40     | 2.46     | 2.53     | 2.66     | 2.21     | 2.79     | 18.1x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 3.1 MB/s, tsv-json 7.4 MB/s, tsv_wasm-json 6.5 MB/s, tsv-internal 83.3 MB/s, tsv_wasm-internal 55.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.2x tsv-internal, tsv_wasm-json 8.5x tsv_wasm-internal

## parse/typescript

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript  | 20.36   | 19  | 49.16    | 49.87    | 51.33    | 53.58    | 54.57    | 47.76    | 54.82    | baseline                      |
| tsv-json          | 31.73   | 28  | 31.59    | 31.93    | 34.19    | 34.61    | 35.69    | 30.60    | 36.04    | 1.56x                         |
| tsv_wasm-json     | 28.86   | 27  | 34.68    | 35.21    | 36.54    | 38.04    | 39.15    | 33.50    | 39.24    | 1.42x                         |
| tsv-internal      | 462.51  | 447 | 2.17     | 2.20     | 2.24     | 2.29     | 2.47     | 2.05     | 2.66     | 22.7x                         |
| tsv_wasm-internal | 305.18  | 292 | 3.27     | 3.35     | 3.45     | 3.58     | 3.89     | 3.10     | 4.04     | 15.0x                         |
| oxc-parser        | 56.31   | 44  | 17.79    | 18.19    | 36.02    | 37.41    | 42.04    | 16.90    | 45.15    | 2.77x                         |
| oxc-parser-wasm   | 44.24   | 39  | 22.24    | 23.86    | 25.41    | 27.67    | 34.04    | 21.27    | 36.72    | 2.17x                         |

**Files (intersection):** 50

**Throughput:** acorn-typescript 5.0 MB/s, tsv-json 7.8 MB/s, tsv_wasm-json 7.1 MB/s, tsv-internal 113.9 MB/s, tsv_wasm-internal 75.2 MB/s, oxc-parser 13.9 MB/s, oxc-parser-wasm 10.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.6x tsv-internal, tsv_wasm-json 10.6x tsv_wasm-internal

## parse/css

| Task Name         | ops/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 252.20  | 229 | 3.95     | 4.10     | 4.41     | 4.58     | 5.02     | 3.73     | 6.58     | baseline                     |
| tsv-json          | 152.62  | 147 | 6.54     | 6.65     | 6.80     | 6.92     | 8.20     | 6.26     | 8.31     | 0.61x                        |
| tsv_wasm-json     | 124.14  | 119 | 8.05     | 8.24     | 8.49     | 9.02     | 9.82     | 7.53     | 10.73    | 0.49x                        |
| tsv-internal      | 399.11  | 393 | 2.51     | 2.55     | 2.57     | 2.60     | 2.78     | 2.38     | 3.19     | 1.58x                        |
| tsv_wasm-internal | 279.23  | 270 | 3.57     | 3.64     | 3.72     | 3.82     | 4.15     | 3.42     | 5.14     | 1.11x                        |

**Files (intersection):** 48

**Throughput:** svelte/compiler 32.3 MB/s, tsv-json 19.5 MB/s, tsv_wasm-json 15.9 MB/s, tsv-internal 51.1 MB/s, tsv_wasm-internal 35.8 MB/s

**Coverage:** svelte/compiler 48/50 (96%), tsv-json 48/50 (96%), tsv_wasm-json 48/50 (96%), tsv-internal 48/50 (96%), tsv_wasm-internal 48/50 (96%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 2.6x tsv-internal, tsv_wasm-json 2.2x tsv_wasm-internal

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.3 MB | 784.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.1 MB | 401.4 KB | 0.4x | 0.5x |
| tsv_wasm | 2.5 MB | 866.1 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 13.6x | 9.5x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 683.6 KB | 0.4x | 0.5x |
| tsv (napi) | 3.4 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.1x | 2.9x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| parse svelte (50f) | **2.43x** svelte |
| parse typescript (50f) | **1.56x** svelte, **0.56x** oxc-parser |
| parse css (48f) | **0.61x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| parse svelte (50f) | **2.14x** svelte |
| parse typescript (50f) | **1.42x** svelte, **0.65x** oxc-parser-wasm |
| parse css (48f) | **0.49x** svelte |

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
