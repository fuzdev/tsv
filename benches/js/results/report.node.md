# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T01:16:33.590Z — tsv 0.2.0 (5950c4ad)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.04       | 11  | 486.23   | 499.56   | 502.25   | 507.28   | 511.30   | 476.86   | 512.31   | baseline                     |
| tsv-json                    | 4.67       | 23  | 213.49   | 215.04   | 218.17   | 218.53   | 219.54   | 212.14   | 219.83   | 2.29x                        |
| tsv_wasm-json               | 4.37       | 19  | 228.79   | 230.61   | 234.34   | 235.01   | 235.79   | 227.10   | 235.99   | 2.14x                        |
| tsv-json-no-locations       | 7.52       | 34  | 132.62   | 134.65   | 136.69   | 137.02   | 137.58   | 131.06   | 137.80   | 3.68x                        |
| tsv_wasm-json-no-locations  | 6.71       | 33  | 148.52   | 150.46   | 151.92   | 152.61   | 153.40   | 146.50   | 153.77   | 3.29x                        |
| tsv-internal                | 49.24      | 202 | 20.20    | 20.79    | 20.96    | 21.05    | 21.40    | 20.03    | 24.60    | 24.1x                        |
| tsv_wasm-internal           | 34.98      | 148 | 28.47    | 29.16    | 29.33    | 29.43    | 29.67    | 28.27    | 30.00    | 17.1x                        |
| rsvelte-parse               | 2.69       | 12  | 372.09   | 373.55   | 380.51   | 382.25   | 382.67   | 368.97   | 382.77   | 1.32x                        |
| rsvelte-parse-skip-expr-loc | 4.58       | 19  | 218.16   | 220.89   | 226.28   | 226.92   | 227.00   | 216.50   | 227.02   | 2.24x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 9.2 MB/s, tsv_wasm-json 8.6 MB/s, tsv-json-no-locations 14.7 MB/s, tsv_wasm-json-no-locations 13.2 MB/s, tsv-internal 96.6 MB/s, tsv_wasm-internal 68.6 MB/s, rsvelte-parse 5.3 MB/s, rsvelte-parse-skip-expr-loc 9.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.5x tsv-internal, tsv_wasm-json 8.0x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22       | 7  | 4433.91  | 4475.28  | 4535.00  | —        | —        | 4377.24  | 4621.46  | baseline              |
| tsv        | 13.14      | 49 | 75.98    | 77.92    | 78.56    | 78.63    | 78.93    | 75.55    | 78.95    | 58.6x                 |
| tsv_wasm   | 9.24       | 39 | 108.06   | 110.28   | 111.84   | 112.28   | 112.48   | 107.22   | 112.57   | 41.2x                 |
| oxfmt      | 0.22       | 6  | 4489.46  | 4528.47  | 4592.22  | —        | —        | 4444.33  | 4659.98  | 0.99x                 |
| biome-wasm | 1.07       | 6  | 934.35   | 935.54   | 947.66   | —        | —        | 904.25   | 959.69   | 4.79x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.4 MB/s, tsv 25.8 MB/s, tsv_wasm 18.1 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.29       | 5  | 3.40    | 3.40    | 3.41    | —       | —       | 3.39    | 3.41    | baseline                      |
| tsv-json                   | 0.48       | 5  | 2.10    | 2.10    | 2.10    | —       | —       | 2.10    | 2.10    | 1.62x                         |
| tsv_wasm-json              | 0.46       | 5  | 2.16    | 2.17    | 2.17    | —       | —       | 2.15    | 2.17    | 1.57x                         |
| tsv-json-no-locations      | 0.97       | 5  | 1.03    | 1.03    | 1.03    | —       | —       | 1.03    | 1.03    | 3.30x                         |
| tsv_wasm-json-no-locations | 0.91       | 5  | 1.10    | 1.11    | 1.11    | —       | —       | 1.10    | 1.11    | 3.07x                         |
| tsv-internal               | 7.04       | 28 | 0.14    | 0.14    | 0.15    | 0.15    | 0.15    | 0.14    | 0.15    | 23.9x                         |
| tsv_wasm-internal          | 5.42       | 24 | 0.18    | 0.19    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 18.4x                         |
| oxc-parser                 | 0.73       | 5  | 1.36    | 1.37    | 1.37    | —       | —       | 1.36    | 1.37    | 2.49x                         |
| oxc-parser-wasm            | 0.72       | 4  | 1.40    | 1.40    | 1.41    | —       | —       | 1.39    | 1.41    | 2.43x                         |
| yuku-parser                | 2.43       | 13 | 0.41    | 0.41    | 0.42    | 0.43    | 0.43    | 0.40    | 0.43    | 8.24x                         |
| yuku-parser-wasm           | 2.88       | 15 | 0.35    | 0.35    | 0.36    | 0.36    | 0.36    | 0.34    | 0.36    | 9.77x                         |
| swc                        | 0.57       | 5  | 1.75    | 1.75    | 1.75    | —       | —       | 1.74    | 1.75    | 1.94x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.6 MB/s, tsv_wasm-json 8.3 MB/s, tsv-json-no-locations 17.4 MB/s, tsv_wasm-json-no-locations 16.3 MB/s, tsv-internal 126.5 MB/s, tsv_wasm-internal 97.4 MB/s, oxc-parser 13.2 MB/s, oxc-parser-wasm 12.9 MB/s, yuku-parser 43.6 MB/s, yuku-parser-wasm 51.7 MB/s, swc 10.3 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.8x tsv-internal, tsv_wasm-json 11.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 6 | 14.12   | 14.13   | 14.20   | —       | —       | 14.02   | 14.30   | baseline              |
| tsv         | 1.73       | 7 | 0.58    | 0.58    | 0.59    | —       | —       | 0.58    | 0.59    | 24.4x                 |
| tsv_wasm    | 1.26       | 6 | 0.79    | 0.79    | 0.80    | —       | —       | 0.79    | 0.81    | 17.8x                 |
| oxfmt       | 1.14       | 6 | 0.88    | 0.88    | 0.88    | —       | —       | 0.86    | 0.88    | 16.1x                 |
| biome-wasm  | 0.22       | 3 | 4.58    | 9.91    | 11.67   | —       | —       | 4.54    | 12.84   | 3.09x                 |
| dprint-wasm | 0.31       | 5 | 3.23    | 3.23    | 3.23    | —       | —       | 3.22    | 3.23    | 4.37x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.3 MB/s, tsv 31.1 MB/s, tsv_wasm 22.7 MB/s, oxfmt 20.6 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), biome-wasm 2506/2506 (100%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 105.84     | 506  | 9.35     | 9.75     | 10.15    | 10.42    | 14.67    | 8.97     | 19.13    | baseline                     |
| tsv-json          | 55.55      | 252  | 17.95    | 18.35    | 18.60    | 20.37    | 20.94    | 17.22    | 21.33    | 0.52x                        |
| tsv_wasm-json     | 53.29      | 228  | 18.74    | 19.10    | 20.37    | 21.21    | 25.27    | 18.36    | 26.44    | 0.50x                        |
| tsv-internal      | 287.29     | 1153 | 3.48     | 3.52     | 3.58     | 3.61     | 3.65     | 3.45     | 7.46     | 2.71x                        |
| tsv_wasm-internal | 202.73     | 824  | 4.93     | 4.98     | 5.03     | 5.06     | 5.11     | 4.86     | 5.78     | 1.92x                        |
| postcss           | 100.07     | 480  | 9.92     | 10.23    | 10.49    | 10.72    | 12.00    | 9.68     | 13.10    | 0.95x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 35.4 MB/s, tsv-json 18.6 MB/s, tsv_wasm-json 17.8 MB/s, tsv-internal 96.1 MB/s, tsv_wasm-internal 67.8 MB/s, postcss 33.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.2x tsv-internal, tsv_wasm-json 3.8x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.81       | 7   | 551.91   | 554.87   | 571.79   | —        | —        | 546.60   | 575.35   | baseline              |
| tsv        | 132.71     | 593 | 7.52     | 7.61     | 7.72     | 7.75     | 7.83     | 7.45     | 11.03    | 73.1x                 |
| tsv_wasm   | 96.21      | 477 | 10.35    | 10.52    | 10.61    | 10.66    | 10.76    | 10.20    | 13.75    | 53.0x                 |
| oxfmt      | 57.99      | 282 | 17.19    | 17.61    | 18.02    | 18.45    | 19.37    | 15.70    | 20.24    | 31.9x                 |
| biome-wasm | 5.30       | 27  | 206.90   | 227.09   | 242.92   | 245.09   | 246.10   | 136.37   | 246.39   | 2.92x                 |
| malva-wasm | 21.85      | 108 | 45.55    | 46.28    | 46.90    | 47.17    | 47.76    | 45.05    | 48.10    | 12.0x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 44.4 MB/s, tsv_wasm 32.2 MB/s, oxfmt 19.4 MB/s, biome-wasm 1.8 MB/s, malva-wasm 7.3 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 838.1 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 928.7 KB | 361.5 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 930.5 KB | — | — |
| biome (wasm) | 44.6 MB | 11.1 MB | 17.8x | 11.9x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.2x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 673.9 KB | 200.3 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.5 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 657.2 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.8x |
| oxc-parser (napi) | 2.1 MB | 885.7 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.0 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 741.1 KB | 310.4 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.3 MB | 3.3 MB | 2.2x | 2.0x |
| rsvelte compiler (napi) | 14.5 MB | 6.0 MB | 3.8x | 3.7x |
| swc (napi) | 31.9 MB | 11.9 MB | 8.3x | 7.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **58.6x** prettier, **59.0x** oxfmt |
| format typescript (2504f) | **24.4x** prettier, **1.51x** oxfmt |
| format css (49f) | **73.1x** prettier, **2.29x** oxfmt |
| parse svelte (773f) | **2.29x** svelte/compiler, **1.74x** rsvelte-parse |
| parse typescript (2503f) | **1.62x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.83x** swc |
| parse css (49f) | **0.52x** svelte/compiler, **0.56x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **41.2x** prettier, **8.60x** biome-wasm |
| format typescript (2504f) | **17.8x** prettier, **5.75x** biome-wasm, **4.07x** dprint-wasm |
| format css (49f) | **53.0x** prettier, **18.2x** biome-wasm, **4.40x** malva-wasm |
| parse svelte (773f) | **2.14x** svelte/compiler |
| parse typescript (2503f) | **1.57x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.16x** yuku-parser-wasm |
| parse css (49f) | **0.50x** svelte/compiler, **0.53x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (the axis a throughput ratio integrates; per component the spread is wider), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) varied more than 10% across iterations (cv = std_dev / mean, post-outlier-removal). Every `Nx` involving one of these divides an unstable mean — read it as approximate, and prefer re-running before drawing a conclusion from it.

| Row | cv | samples |
| --- | ---: | ---: |
| format/css/biome-wasm | 24.0% | 27 |

## Skipped Files

3 files skipped, 12 unique file+error combinations — Svelte 0, TypeScript 3, CSS 0 files.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
