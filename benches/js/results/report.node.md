# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T22:16:26.009Z — tsv 0.2.0 (3d0eea49)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.03       | 11  | 488.62   | 500.84   | 501.50   | 508.45   | 514.00   | 481.68   | 515.39   | baseline                     |
| tsv-json                    | 4.63       | 19  | 216.38   | 217.43   | 221.41   | 221.72   | 222.05   | 215.11   | 222.15   | 2.28x                        |
| tsv_wasm-json               | 4.32       | 19  | 231.20   | 233.21   | 236.98   | 237.52   | 237.86   | 229.89   | 237.95   | 2.13x                        |
| tsv-json-no-locations       | 7.48       | 33  | 133.60   | 134.81   | 137.27   | 137.61   | 138.07   | 132.18   | 138.12   | 3.69x                        |
| tsv_wasm-json-no-locations  | 6.65       | 30  | 150.10   | 151.77   | 153.92   | 154.83   | 155.15   | 148.38   | 155.20   | 3.28x                        |
| tsv-internal                | 49.17      | 200 | 20.26    | 20.83    | 21.04    | 21.13    | 21.25    | 20.07    | 21.31    | 24.2x                        |
| tsv_wasm-internal           | 34.70      | 129 | 28.76    | 29.48    | 29.76    | 29.84    | 30.00    | 28.54    | 30.31    | 17.1x                        |
| rsvelte-parse               | 2.67       | 12  | 374.86   | 376.97   | 385.17   | 386.80   | 386.89   | 372.94   | 386.91   | 1.31x                        |
| rsvelte-parse-skip-expr-loc | 4.53       | 22  | 219.92   | 221.84   | 225.53   | 226.16   | 228.15   | 218.58   | 228.70   | 2.23x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 9.1 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 14.7 MB/s, tsv_wasm-json-no-locations 13.0 MB/s, tsv-internal 96.4 MB/s, tsv_wasm-internal 68.1 MB/s, rsvelte-parse 5.2 MB/s, rsvelte-parse-skip-expr-loc 8.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.6x tsv-internal, tsv_wasm-json 8.0x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22       | 7  | 4482.16  | 4560.61  | 4632.02  | —        | —        | 4424.31  | 4711.23  | baseline              |
| tsv        | 13.19      | 61 | 75.44    | 77.30    | 77.92    | 78.28    | 78.52    | 74.84    | 78.71    | 59.6x                 |
| tsv_wasm   | 9.29       | 44 | 107.08   | 108.90   | 110.63   | 111.14   | 111.92   | 106.23   | 112.22   | 42.0x                 |
| oxfmt      | 0.22       | 5  | 4510.90  | 4544.21  | 4617.21  | —        | —        | 4497.26  | 4690.76  | 1.00x                 |
| biome-wasm | 1.04       | 6  | 963.21   | 967.27   | 983.40   | —        | —        | 949.13   | 999.03   | 4.68x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.4 MB/s, tsv 25.9 MB/s, tsv_wasm 18.2 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.0 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.30       | 5  | 3.34    | 3.34    | 3.35    | —       | —       | 3.33    | 3.35    | baseline                      |
| tsv-json                   | 0.47       | 5  | 2.13    | 2.13    | 2.13    | —       | —       | 2.13    | 2.13    | 1.57x                         |
| tsv_wasm-json              | 0.45       | 5  | 2.20    | 2.21    | 2.21    | —       | —       | 2.19    | 2.21    | 1.52x                         |
| tsv-json-no-locations      | 0.97       | 4  | 1.03    | 1.03    | 1.03    | —       | —       | 1.03    | 1.03    | 3.23x                         |
| tsv_wasm-json-no-locations | 0.90       | 5  | 1.12    | 1.12    | 1.12    | —       | —       | 1.11    | 1.12    | 2.99x                         |
| tsv-internal               | 6.98       | 29 | 0.14    | 0.14    | 0.15    | 0.15    | 0.15    | 0.14    | 0.15    | 23.3x                         |
| tsv_wasm-internal          | 5.23       | 22 | 0.19    | 0.19    | 0.19    | 0.20    | 0.20    | 0.19    | 0.20    | 17.5x                         |
| oxc-parser                 | 0.73       | 5  | 1.37    | 1.37    | 1.38    | —       | —       | 1.36    | 1.38    | 2.44x                         |
| oxc-parser-wasm            | 0.71       | 4  | 1.41    | 1.41    | 1.42    | —       | —       | 1.41    | 1.42    | 2.36x                         |
| yuku-parser                | 2.43       | 13 | 0.41    | 0.41    | 0.42    | 0.42    | 0.42    | 0.40    | 0.42    | 8.13x                         |
| yuku-parser-wasm           | 2.86       | 13 | 0.35    | 0.35    | 0.36    | 0.37    | 0.37    | 0.35    | 0.37    | 9.55x                         |
| swc                        | 0.57       | 4  | 1.76    | 1.76    | 1.77    | —       | —       | 1.76    | 1.77    | 1.90x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.4 MB/s, tsv_wasm-json 8.2 MB/s, tsv-json-no-locations 17.4 MB/s, tsv_wasm-json-no-locations 16.1 MB/s, tsv-internal 125.5 MB/s, tsv_wasm-internal 94.0 MB/s, oxc-parser 13.1 MB/s, oxc-parser-wasm 12.7 MB/s, yuku-parser 43.8 MB/s, yuku-parser-wasm 51.4 MB/s, swc 10.2 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.9x tsv-internal, tsv_wasm-json 11.5x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 7 | 14.47   | 14.54   | 14.62   | —       | —       | 14.37   | 14.66   | baseline              |
| tsv         | 1.72       | 7 | 0.58    | 0.58    | 0.59    | —       | —       | 0.58    | 0.59    | 25.0x                 |
| tsv_wasm    | 1.26       | 6 | 0.80    | 0.80    | 0.80    | —       | —       | 0.79    | 0.81    | 18.2x                 |
| oxfmt       | 1.12       | 6 | 0.90    | 0.90    | 0.91    | —       | —       | 0.88    | 0.91    | 16.2x                 |
| biome-wasm  | 0.22       | 3 | 4.63    | 10.17   | 11.78   | —       | —       | 4.59    | 12.85   | 3.14x                 |
| dprint-wasm | 0.31       | 3 | 3.24    | 3.24    | 3.25    | —       | —       | 3.24    | 3.25    | 4.47x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.2 MB/s, tsv 31.0 MB/s, tsv_wasm 22.6 MB/s, oxfmt 20.1 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.5 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), biome-wasm 2506/2506 (100%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 106.40     | 501  | 9.30     | 9.69     | 10.11    | 10.38    | 15.61    | 8.96     | 18.85    | baseline                     |
| tsv-json          | 55.37      | 251  | 18.01    | 18.46    | 18.69    | 20.64    | 21.45    | 17.20    | 22.11    | 0.52x                        |
| tsv_wasm-json     | 53.11      | 235  | 18.77    | 19.19    | 20.31    | 20.86    | 25.57    | 18.31    | 26.37    | 0.50x                        |
| tsv-internal      | 300.01     | 1208 | 3.33     | 3.37     | 3.43     | 3.46     | 3.52     | 3.31     | 3.80     | 2.82x                        |
| tsv_wasm-internal | 205.19     | 824  | 4.87     | 4.93     | 5.00     | 5.05     | 5.09     | 4.80     | 8.88     | 1.93x                        |
| postcss           | 100.26     | 473  | 9.91     | 10.21    | 10.51    | 10.83    | 11.84    | 9.67     | 13.81    | 0.94x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 35.6 MB/s, tsv-json 18.5 MB/s, tsv_wasm-json 17.8 MB/s, tsv-internal 100.4 MB/s, tsv_wasm-internal 68.7 MB/s, postcss 33.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.4x tsv-internal, tsv_wasm-json 3.9x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.75       | 9   | 568.16   | 588.19   | 593.56   | —        | —        | 554.97   | 596.92   | baseline              |
| tsv        | 141.45     | 630 | 7.05     | 7.14     | 7.27     | 7.31     | 7.49     | 6.98     | 10.65    | 81.1x                 |
| tsv_wasm   | 98.12      | 487 | 10.15    | 10.32    | 10.41    | 10.45    | 10.54    | 10.02    | 13.34    | 56.2x                 |
| oxfmt      | 57.12      | 284 | 17.51    | 17.88    | 18.14    | 18.41    | 19.38    | 16.17    | 19.98    | 32.7x                 |
| biome-wasm | 6.13       | 22  | 141.47   | 223.67   | 241.91   | 244.80   | 246.93   | 137.03   | 247.24   | 3.51x                 |
| malva-wasm | 21.90      | 102 | 45.47    | 46.25    | 46.87    | 47.40    | 48.03    | 45.02    | 48.41    | 12.6x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 47.4 MB/s, tsv_wasm 32.8 MB/s, oxfmt 19.1 MB/s, biome-wasm 2.1 MB/s, malva-wasm 7.3 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 839.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 928.7 KB | 361.5 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 932.3 KB | — | — |
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
| format svelte (773f) | **59.6x** prettier, **59.4x** oxfmt |
| format typescript (2504f) | **25.0x** prettier, **1.54x** oxfmt |
| format css (49f) | **81.1x** prettier, **2.48x** oxfmt |
| parse svelte (773f) | **2.28x** svelte/compiler, **1.74x** rsvelte-parse |
| parse typescript (2503f) | **1.57x** acorn-typescript, **0.64x** oxc-parser, **0.19x** yuku-parser, **0.83x** swc |
| parse css (49f) | **0.52x** svelte/compiler, **0.55x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **42.0x** prettier, **8.97x** biome-wasm |
| format typescript (2504f) | **18.2x** prettier, **5.79x** biome-wasm, **4.08x** dprint-wasm |
| format css (49f) | **56.2x** prettier, **16.0x** biome-wasm, **4.48x** malva-wasm |
| parse svelte (773f) | **2.13x** svelte/compiler |
| parse typescript (2503f) | **1.52x** acorn-typescript, **0.64x** oxc-parser-wasm, **0.16x** yuku-parser-wasm |
| parse css (49f) | **0.50x** svelte/compiler, **0.53x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (the axis a throughput ratio integrates; per component the spread is wider), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) varied more than 10% across iterations (cv = std_dev / mean, post-outlier-removal). Every `Nx` involving one of these divides an unstable mean — read it as approximate, and prefer re-running before drawing a conclusion from it.

| Row | cv | samples |
| --- | ---: | ---: |
| format/css/biome-wasm | 22.8% | 22 |

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
