# tsv benchmark results

**Runtime:** deno

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-07-06T22:27:28.070Z — tsv 0.1.0 (a99ef299)

**Corpus:** 762 Svelte (1.8 MB), 2302 TypeScript (16.1 MB), 50 CSS (0.3 MB) — 3114 files, 18.2 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (62), ../fuz_css/src (122), ../fuz_docs/src (64), ../fuz_gitops/src (98), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../mdz/src (58), ../gro/src (155), ../svelte-docinfo/src (98), ../tsv.fuz.dev/src (27), ../ryanatkn.com/src (51), ../webdevladder.net/src (38), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (273), ../svelte/packages/svelte/src (380), ../svelte.dev/apps/svelte.dev/src (138), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65)

**Versions:** svelte@5.56.4, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.139.0, oxfmt@0.57.0, @biomejs/wasm-bundler@2.5.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| -------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler            | 2.34       | 11  | 425.20   | 434.60   | 441.95   | 447.66   | 452.58   | 419.24   | 453.81   | baseline                     |
| tsv-json                   | 5.12       | 26  | 194.90   | 196.19   | 199.42   | 199.65   | 199.71   | 193.39   | 199.72   | 2.19x                        |
| tsv-json-no-locations      | 7.96       | 40  | 124.60   | 127.41   | 128.55   | 128.90   | 129.41   | 123.34   | 129.57   | 3.41x                        |
| tsv_wasm-json              | 4.21       | 20  | 237.13   | 238.17   | 241.60   | 243.53   | 243.69   | 235.48   | 243.70   | 1.80x                        |
| tsv_wasm-json-no-locations | 6.33       | 30  | 157.56   | 158.71   | 161.57   | 162.34   | 163.49   | 156.00   | 163.82   | 2.71x                        |
| tsv-internal               | 45.96      | 227 | 21.60    | 22.15    | 22.39    | 22.54    | 23.34    | 21.27    | 26.64    | 19.7x                        |
| tsv_wasm-internal          | 29.76      | 147 | 33.37    | 34.14    | 34.42    | 34.56    | 35.01    | 32.94    | 35.94    | 12.7x                        |

**Files (intersection):** 762

**Throughput:** svelte/compiler 4.3 MB/s, tsv-json 9.4 MB/s, tsv-json-no-locations 14.7 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 11.7 MB/s, tsv-internal 84.6 MB/s, tsv_wasm-internal 54.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.0x tsv-internal, tsv_wasm-json 7.1x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.24       | 7  | 4168.62  | 4208.71  | 4247.17  | —        | —        | 4130.43  | 4283.93  | baseline              |
| tsv        | 12.69      | 64 | 78.41    | 79.55    | 80.30    | 80.43    | 80.78    | 77.71    | 80.82    | 53.1x                 |
| tsv_wasm   | 8.20       | 41 | 121.22   | 123.21   | 124.62   | 125.51   | 125.92   | 119.87   | 126.14   | 34.3x                 |
| oxfmt      | 0.25       | 5  | 4052.94  | 4065.13  | 4076.69  | —        | —        | 4032.16  | 4084.39  | 1.03x                 |
| biome-wasm | 1.46       | 7  | 685.63   | 687.73   | 695.90   | —        | —        | 682.22   | 707.68   | 6.10x                 |

**Files (intersection):** 762

**Throughput:** prettier 0.4 MB/s, tsv 23.3 MB/s, tsv_wasm 15.1 MB/s, oxfmt 0.5 MB/s, biome-wasm 2.7 MB/s

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.37       | 5  | 2.70    | 2.70    | 2.70    | —       | —       | 2.69    | 2.70    | baseline                      |
| tsv-json                   | 0.57       | 4  | 1.75    | 1.75    | 1.75    | —       | —       | 1.74    | 1.76    | 1.54x                         |
| tsv-json-no-locations      | 1.18       | 5  | 0.85    | 0.86    | 0.87    | —       | —       | 0.85    | 0.88    | 3.17x                         |
| tsv_wasm-json              | 0.49       | 5  | 2.05    | 2.05    | 2.06    | —       | —       | 2.04    | 2.06    | 1.31x                         |
| tsv_wasm-json-no-locations | 0.97       | 3  | 1.03    | 1.05    | 1.06    | —       | —       | 1.03    | 1.07    | 2.61x                         |
| tsv-internal               | 7.43       | 30 | 0.13    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 20.0x                         |
| tsv_wasm-internal          | 4.95       | 20 | 0.20    | 0.20    | 0.21    | 0.21    | 0.21    | 0.20    | 0.21    | 13.3x                         |
| oxc-parser                 | 0.89       | 5  | 1.14    | 1.14    | 1.15    | —       | —       | 1.10    | 1.16    | 2.39x                         |
| oxc-parser-wasm            | 0.79       | 5  | 1.27    | 1.28    | 1.29    | —       | —       | 1.26    | 1.29    | 2.12x                         |

**Files (intersection):** 2302

**Throughput:** acorn-typescript 6.0 MB/s, tsv-json 9.2 MB/s, tsv-json-no-locations 18.9 MB/s, tsv_wasm-json 7.8 MB/s, tsv_wasm-json-no-locations 15.6 MB/s, tsv-internal 119.3 MB/s, tsv_wasm-internal 79.4 MB/s, oxc-parser 14.2 MB/s, oxc-parser-wasm 12.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.0x tsv-internal, tsv_wasm-json 10.1x tsv_wasm-internal

## format/typescript

| Task Name  | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.08       | 7 | 11936.36 | 11982.40 | 11994.91 | —        | —        | 11837.97 | 12010.39 | baseline              |
| tsv        | 1.91       | 9 | 523.14   | 525.01   | 528.51   | —        | —        | 521.64   | 535.70   | 22.8x                 |
| tsv_wasm   | 1.24       | 6 | 803.13   | 804.47   | 809.02   | —        | —        | 802.71   | 814.95   | 14.8x                 |
| oxfmt      | 1.14       | 6 | 879.08   | 882.05   | 886.41   | —        | —        | 870.54   | 889.83   | 13.6x                 |
| biome-wasm | 0.25       | 5 | 3946.12  | 3955.42  | 3966.57  | —        | —        | 3935.82  | 3974.00  | 3.02x                 |

**Files (intersection):** 2302

**Throughput:** prettier 1.3 MB/s, tsv 30.7 MB/s, tsv_wasm 20.0 MB/s, oxfmt 18.3 MB/s, biome-wasm 4.1 MB/s

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 109.14     | 545 | 9.11     | 9.37     | 9.60     | 9.82     | 10.21    | 8.69     | 10.77    | baseline                     |
| tsv-json          | 58.73      | 264 | 16.99    | 17.37    | 17.69    | 18.90    | 19.54    | 16.65    | 28.61    | 0.54x                        |
| tsv_wasm-json     | 45.02      | 217 | 22.12    | 22.57    | 23.04    | 23.43    | 24.25    | 21.70    | 24.68    | 0.41x                        |
| tsv-internal      | 192.15     | 836 | 5.20     | 5.24     | 5.32     | 5.37     | 5.47     | 5.15     | 6.22     | 1.76x                        |
| tsv_wasm-internal | 117.42     | 536 | 8.49     | 8.59     | 8.66     | 8.68     | 8.78     | 8.45     | 8.97     | 1.08x                        |

**Files (intersection):** 50

**Throughput:** svelte/compiler 32.6 MB/s, tsv-json 17.6 MB/s, tsv_wasm-json 13.5 MB/s, tsv-internal 57.4 MB/s, tsv_wasm-internal 35.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 3.3x tsv-internal, tsv_wasm-json 2.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.07       | 11  | 480.02   | 491.62   | 495.64   | 497.67   | 499.30   | 474.51   | 499.71   | baseline              |
| tsv        | 107.15     | 495 | 9.29     | 9.45     | 9.58     | 9.63     | 9.89     | 9.21     | 10.30    | 51.9x                 |
| tsv_wasm   | 64.49      | 318 | 15.45    | 15.63    | 15.72    | 15.78    | 16.03    | 15.32    | 16.36    | 31.2x                 |
| oxfmt      | 10.75      | 54  | 93.43    | 97.57    | 99.16    | 101.02   | 102.29   | 83.12    | 102.46   | 5.20x                 |
| biome-wasm | 10.85      | 51  | 91.85    | 93.48    | 94.45    | 96.21    | 101.47   | 90.31    | 105.65   | 5.25x                 |

**Files (intersection):** 50

**Throughput:** prettier 0.6 MB/s, tsv 32.0 MB/s, tsv_wasm 19.3 MB/s, oxfmt 3.2 MB/s, biome-wasm 3.2 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 762.6 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 381.0 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 841.8 KB | — | — |
| biome (wasm) | 37.5 MB | 9.0 MB | 15.4x | 10.7x |
| oxc-parser (wasm) | 1.6 MB | 501.4 KB | 0.7x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.5 MB | 4.6 MB | 3.4x | 3.2x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 691.2 KB | 0.5x | 0.5x |
| tsv (napi) | 3.5 MB | 1.5 MB | 1.0x | 1.0x |
| oxc-parser (napi) | 2.4 MB | 977.4 KB | 0.7x | 0.7x |
| oxfmt (napi) | 9.1 MB | 3.6 MB | 2.7x | 2.5x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **53.1x** prettier, **51.5x** oxfmt |
| format typescript (2302f) | **22.8x** prettier, **1.68x** oxfmt |
| format css (50f) | **51.9x** prettier, **9.97x** oxfmt |
| parse svelte (762f) | **2.19x** svelte |
| parse typescript (2302f) | **1.54x** svelte, **0.65x** oxc-parser |
| parse css (50f) | **0.54x** svelte |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (762f) | **34.3x** prettier, **5.62x** biome-wasm |
| format typescript (2302f) | **14.8x** prettier, **4.92x** biome-wasm |
| format css (50f) | **31.2x** prettier, **5.94x** biome-wasm |
| parse svelte (762f) | **1.80x** svelte |
| parse typescript (2302f) | **1.31x** svelte, **0.62x** oxc-parser-wasm |
| parse css (50f) | **0.41x** svelte |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for .svelte/.css, acorn-typescript for .ts. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no oxc counterpart — oxc exposes no comparably cheap mode (its JS API always serializes; experimentalLazy is setup-dominated). Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in `@fuzdev/tsv_parse_wasm` / `@fuzdev/tsv_wasm`) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._
