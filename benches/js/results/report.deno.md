# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.5

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T15:28:09.910Z — tsv 0.2.0 (831e5193)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.06       | 11  | 481.25   | 491.98   | 512.40   | 514.03   | 515.34   | 465.35   | 515.66   | baseline                     |
| tsv-json                    | 5.19       | 24  | 192.48   | 194.29   | 197.53   | 198.49   | 206.55   | 190.83   | 209.16   | 2.52x                        |
| tsv_wasm-json               | 4.39       | 22  | 226.95   | 228.91   | 231.16   | 232.56   | 233.00   | 225.16   | 233.09   | 2.13x                        |
| tsv-json-no-locations       | 8.12       | 40  | 122.54   | 124.31   | 126.17   | 127.41   | 127.88   | 121.28   | 128.16   | 3.94x                        |
| tsv_wasm-json-no-locations  | 6.59       | 32  | 151.20   | 152.67   | 155.49   | 155.55   | 155.89   | 149.89   | 156.04   | 3.20x                        |
| tsv-internal                | 50.07      | 182 | 19.91    | 20.57    | 20.80    | 20.93    | 21.48    | 19.80    | 21.79    | 24.3x                        |
| tsv_wasm-internal           | 32.02      | 127 | 31.12    | 32.04    | 32.19    | 32.23    | 32.32    | 30.94    | 32.61    | 15.6x                        |
| rsvelte-parse               | 2.86       | 14  | 348.40   | 351.39   | 355.34   | 357.71   | 359.64   | 345.41   | 360.12   | 1.39x                        |
| rsvelte-parse-skip-expr-loc | 4.83       | 20  | 207.16   | 209.23   | 214.53   | 216.15   | 216.85   | 205.41   | 216.98   | 2.34x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 10.2 MB/s, tsv_wasm-json 8.6 MB/s, tsv-json-no-locations 15.9 MB/s, tsv_wasm-json-no-locations 12.9 MB/s, tsv-internal 98.2 MB/s, tsv_wasm-internal 62.8 MB/s, rsvelte-parse 5.6 MB/s, rsvelte-parse-skip-expr-loc 9.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.7x tsv-internal, tsv_wasm-json 7.3x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 6  | 4405.90  | 4420.63  | 4575.10  | —        | —        | 4329.07  | 4785.94  | baseline              |
| tsv        | 13.00      | 60 | 76.51    | 78.71    | 80.11    | 80.61    | 81.14    | 75.47    | 81.69    | 57.0x                 |
| tsv_wasm   | 8.49       | 33 | 117.81   | 119.99   | 121.63   | 122.26   | 122.97   | 117.03   | 123.04   | 37.2x                 |
| oxfmt      | 0.23       | 7  | 4389.17  | 4432.03  | 4547.88  | —        | —        | 4313.78  | 4657.50  | 0.99x                 |
| biome-wasm | 1.31       | 6  | 761.31   | 763.14   | 770.24   | —        | —        | 757.68   | 780.04   | 5.76x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.4 MB/s, tsv 25.5 MB/s, tsv_wasm 16.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.32       | 5  | 3.18    | 3.18    | 3.19    | —       | —       | 3.15    | 3.19    | baseline                      |
| tsv-json                   | 0.54       | 5  | 1.84    | 1.85    | 1.85    | —       | —       | 1.83    | 1.85    | 1.72x                         |
| tsv_wasm-json              | 0.48       | 5  | 2.07    | 2.08    | 2.08    | —       | —       | 2.06    | 2.08    | 1.53x                         |
| tsv-json-no-locations      | 1.10       | 5  | 0.91    | 0.91    | 0.91    | —       | —       | 0.91    | 0.92    | 3.50x                         |
| tsv_wasm-json-no-locations | 0.92       | 4  | 1.09    | 1.09    | 1.09    | —       | —       | 1.08    | 1.10    | 2.92x                         |
| tsv-internal               | 7.53       | 28 | 0.13    | 0.13    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 23.9x                         |
| tsv_wasm-internal          | 5.03       | 21 | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 15.9x                         |
| oxc-parser                 | 0.82       | 5  | 1.23    | 1.23    | 1.23    | —       | —       | 1.20    | 1.24    | 2.60x                         |
| oxc-parser-wasm            | 0.74       | 4  | 1.35    | 1.35    | 1.35    | —       | —       | 1.34    | 1.35    | 2.36x                         |
| yuku-parser                | 2.23       | 11 | 0.44    | 0.46    | 0.47    | 0.49    | 0.50    | 0.43    | 0.50    | 7.08x                         |
| yuku-parser-wasm           | 2.46       | 13 | 0.41    | 0.42    | 0.42    | 0.42    | 0.43    | 0.38    | 0.43    | 7.80x                         |
| swc                        | 0.60       | 5  | 1.67    | 1.67    | 1.67    | —       | —       | 1.66    | 1.67    | 1.90x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 5.7 MB/s, tsv-json 9.8 MB/s, tsv_wasm-json 8.7 MB/s, tsv-json-no-locations 19.8 MB/s, tsv_wasm-json-no-locations 16.5 MB/s, tsv-internal 135.4 MB/s, tsv_wasm-internal 90.3 MB/s, oxc-parser 14.8 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 40.2 MB/s, yuku-parser-wasm 44.2 MB/s, swc 10.8 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.9x tsv-internal, tsv_wasm-json 10.4x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 6 | 12.80   | 12.91   | 13.08   | —       | —       | 12.69   | 13.19   | baseline              |
| tsv         | 1.76       | 7 | 0.57    | 0.57    | 0.58    | —       | —       | 0.57    | 0.59    | 22.5x                 |
| tsv_wasm    | 1.15       | 5 | 0.87    | 0.87    | 0.87    | —       | —       | 0.87    | 0.88    | 14.7x                 |
| oxfmt       | 1.16       | 5 | 0.86    | 0.86    | 0.86    | —       | —       | 0.86    | 0.87    | 14.9x                 |
| biome-wasm  | 0.23       | 5 | 4.30    | 4.31    | 4.31    | —       | —       | 4.30    | 4.31    | 2.98x                 |
| dprint-wasm | 0.28       | 5 | 3.61    | 3.62    | 3.62    | —       | —       | 3.61    | 3.62    | 3.54x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.4 MB/s, tsv 31.6 MB/s, tsv_wasm 20.7 MB/s, oxfmt 20.9 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), biome-wasm 2506/2506 (100%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 109.94     | 547  | 9.04     | 9.37     | 9.61     | 9.77     | 10.25    | 8.56     | 11.98    | baseline                     |
| tsv-json          | 63.79      | 311  | 15.56    | 16.02    | 16.21    | 16.63    | 17.72    | 15.05    | 17.97    | 0.58x                        |
| tsv_wasm-json     | 51.16      | 251  | 19.46    | 19.83    | 20.02    | 20.43    | 21.52    | 19.08    | 21.79    | 0.47x                        |
| tsv-internal      | 311.47     | 1384 | 3.20     | 3.24     | 3.29     | 3.32     | 3.36     | 3.15     | 3.82     | 2.83x                        |
| tsv_wasm-internal | 173.94     | 622  | 5.75     | 5.81     | 5.88     | 5.90     | 5.93     | 5.73     | 6.24     | 1.58x                        |
| postcss           | 99.31      | 486  | 9.99     | 10.27    | 10.66    | 10.95    | 11.57    | 9.64     | 13.15    | 0.90x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 36.8 MB/s, tsv-json 21.4 MB/s, tsv_wasm-json 17.1 MB/s, tsv-internal 104.3 MB/s, tsv_wasm-internal 58.2 MB/s, postcss 33.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.9x tsv-internal, tsv_wasm-json 3.4x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.96       | 9   | 507.31   | 523.61   | 529.97   | —        | —        | 501.52   | 533.11   | baseline              |
| tsv        | 149.81     | 645 | 6.66     | 6.77     | 6.89     | 6.94     | 7.05     | 6.59     | 10.91    | 76.5x                 |
| tsv_wasm   | 85.90      | 318 | 11.63    | 11.85    | 11.94    | 11.96    | 12.07    | 11.56    | 12.25    | 43.9x                 |
| oxfmt      | 58.09      | 288 | 17.20    | 17.56    | 17.89    | 18.34    | 18.77    | 15.64    | 21.68    | 29.7x                 |
| biome-wasm | 9.93       | 40  | 100.65   | 102.55   | 103.33   | 103.97   | 104.40   | 99.90    | 104.61   | 5.07x                 |
| malva-wasm | 19.76      | 93  | 50.38    | 51.19    | 51.51    | 51.71    | 54.01    | 50.07    | 54.57    | 10.1x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.7 MB/s, tsv 50.2 MB/s, tsv_wasm 28.8 MB/s, oxfmt 19.4 MB/s, biome-wasm 3.3 MB/s, malva-wasm 6.6 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 839.8 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 928.7 KB | 361.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 932.2 KB | — | — |
| biome (wasm) | 44.6 MB | 11.1 MB | 17.8x | 11.9x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.2x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 673.9 KB | 200.3 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 3.0x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 657.5 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 885.7 KB | 0.6x | 0.6x |
| oxfmt (napi) | 9.0 MB | 3.7 MB | 2.6x | 2.4x |
| yuku-parser (napi) | 741.1 KB | 310.4 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.3 MB | 3.3 MB | 2.4x | 2.1x |
| rsvelte compiler (napi) | 14.5 MB | 6.0 MB | 4.1x | 3.9x |
| swc (napi) | 31.9 MB | 11.9 MB | 9.1x | 7.8x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **57.0x** prettier, **57.4x** oxfmt |
| format typescript (2504f) | **22.5x** prettier, **1.51x** oxfmt |
| format css (49f) | **76.5x** prettier, **2.58x** oxfmt |
| parse svelte (773f) | **2.52x** svelte/compiler, **1.81x** rsvelte-parse |
| parse typescript (2503f) | **1.72x** acorn-typescript, **0.66x** oxc-parser, **0.24x** yuku-parser, **0.91x** swc |
| parse css (49f) | **0.58x** svelte/compiler, **0.64x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **37.2x** prettier, **6.46x** biome-wasm |
| format typescript (2504f) | **14.7x** prettier, **4.95x** biome-wasm, **4.15x** dprint-wasm |
| format css (49f) | **43.9x** prettier, **8.65x** biome-wasm, **4.35x** malva-wasm |
| parse svelte (773f) | **2.13x** svelte/compiler |
| parse typescript (2503f) | **1.53x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (49f) | **0.47x** svelte/compiler, **0.52x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (the axis a throughput ratio integrates; per component the spread is wider), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

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
