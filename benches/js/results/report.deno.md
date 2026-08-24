# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.5

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T01:05:57.258Z — tsv 0.2.0 (5950c4ad)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.08       | 10  | 476.78   | 486.24   | 511.87   | 513.31   | 514.46   | 468.49   | 514.75   | baseline                     |
| tsv-json                    | 5.18       | 23  | 192.81   | 195.50   | 198.62   | 199.77   | 200.22   | 191.30   | 200.35   | 2.48x                        |
| tsv_wasm-json               | 4.39       | 18  | 227.96   | 229.16   | 233.21   | 233.64   | 234.59   | 226.82   | 234.84   | 2.11x                        |
| tsv-json-no-locations       | 8.16       | 36  | 122.08   | 125.15   | 125.74   | 126.96   | 127.63   | 121.14   | 127.91   | 3.92x                        |
| tsv_wasm-json-no-locations  | 6.52       | 32  | 152.34   | 154.80   | 157.48   | 157.63   | 158.24   | 151.13   | 158.46   | 3.13x                        |
| tsv-internal                | 49.72      | 173 | 20.10    | 20.69    | 20.95    | 21.06    | 21.48    | 19.97    | 21.72    | 23.9x                        |
| tsv_wasm-internal           | 31.91      | 122 | 31.28    | 31.87    | 32.13    | 32.24    | 32.32    | 31.09    | 32.34    | 15.3x                        |
| rsvelte-parse               | 2.86       | 14  | 349.94   | 351.43   | 358.62   | 361.71   | 363.92   | 346.47   | 364.47   | 1.37x                        |
| rsvelte-parse-skip-expr-loc | 4.83       | 20  | 207.21   | 209.69   | 214.39   | 214.98   | 216.17   | 205.18   | 216.49   | 2.32x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.1 MB/s, tsv-json 10.2 MB/s, tsv_wasm-json 8.6 MB/s, tsv-json-no-locations 16.0 MB/s, tsv_wasm-json-no-locations 12.8 MB/s, tsv-internal 97.5 MB/s, tsv_wasm-internal 62.6 MB/s, rsvelte-parse 5.6 MB/s, rsvelte-parse-skip-expr-loc 9.5 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.6x tsv-internal, tsv_wasm-json 7.3x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4344.76  | 4432.40  | 4520.27  | —        | —        | 4188.07  | 4551.17  | baseline              |
| tsv        | 13.12      | 59 | 75.95    | 77.96    | 79.20    | 79.42    | 80.13    | 75.27    | 80.60    | 57.0x                 |
| tsv_wasm   | 8.46       | 40 | 117.81   | 119.03   | 121.17   | 122.51   | 123.11   | 116.95   | 123.14   | 36.8x                 |
| oxfmt      | 0.23       | 7  | 4396.52  | 4414.93  | 4515.41  | —        | —        | 4272.07  | 4650.08  | 0.99x                 |
| biome-wasm | 1.33       | 6  | 753.98   | 757.28   | 764.73   | —        | —        | 752.44   | 774.85   | 5.76x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.5 MB/s, tsv 25.7 MB/s, tsv_wasm 16.6 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 5  | 3.19    | 3.19    | 3.20    | —       | —       | 3.17    | 3.20    | baseline                      |
| tsv-json                   | 0.54       | 4  | 1.83    | 1.84    | 1.85    | —       | —       | 1.83    | 1.85    | 1.74x                         |
| tsv_wasm-json              | 0.48       | 4  | 2.07    | 2.08    | 2.09    | —       | —       | 2.07    | 2.10    | 1.54x                         |
| tsv-json-no-locations      | 1.10       | 6  | 0.91    | 0.91    | 0.91    | —       | —       | 0.90    | 0.91    | 3.52x                         |
| tsv_wasm-json-no-locations | 0.92       | 4  | 1.09    | 1.09    | 1.10    | —       | —       | 1.09    | 1.10    | 2.93x                         |
| tsv-internal               | 7.53       | 30 | 0.13    | 0.13    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 24.0x                         |
| tsv_wasm-internal          | 5.02       | 22 | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 16.0x                         |
| oxc-parser                 | 0.82       | 5  | 1.21    | 1.24    | 1.24    | —       | —       | 1.19    | 1.24    | 2.62x                         |
| oxc-parser-wasm            | 0.74       | 3  | 1.34    | 1.34    | 1.34    | —       | —       | 1.34    | 1.34    | 2.37x                         |
| yuku-parser                | 2.20       | 11 | 0.46    | 0.46    | 0.46    | 0.47    | 0.47    | 0.44    | 0.47    | 7.00x                         |
| yuku-parser-wasm           | 2.45       | 13 | 0.41    | 0.42    | 0.42    | 0.42    | 0.43    | 0.39    | 0.43    | 7.80x                         |
| swc                        | 0.61       | 4  | 1.63    | 1.63    | 1.63    | —       | —       | 1.63    | 1.63    | 1.95x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 9.8 MB/s, tsv_wasm-json 8.7 MB/s, tsv-json-no-locations 19.8 MB/s, tsv_wasm-json-no-locations 16.5 MB/s, tsv-internal 135.3 MB/s, tsv_wasm-internal 90.3 MB/s, oxc-parser 14.7 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 39.5 MB/s, yuku-parser-wasm 44.0 MB/s, swc 11.0 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.8x tsv-internal, tsv_wasm-json 10.4x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 7 | 12.75   | 12.91   | 13.02   | —       | —       | 12.66   | 13.05   | baseline              |
| tsv         | 1.78       | 7 | 0.56    | 0.56    | 0.57    | —       | —       | 0.56    | 0.58    | 22.8x                 |
| tsv_wasm    | 1.16       | 5 | 0.86    | 0.86    | 0.87    | —       | —       | 0.86    | 0.87    | 14.8x                 |
| oxfmt       | 1.15       | 6 | 0.87    | 0.87    | 0.87    | —       | —       | 0.86    | 0.87    | 14.8x                 |
| biome-wasm  | 0.23       | 5 | 4.28    | 4.28    | 4.28    | —       | —       | 4.27    | 4.28    | 3.00x                 |
| dprint-wasm | 0.28       | 5 | 3.63    | 3.63    | 3.63    | —       | —       | 3.63    | 3.63    | 3.53x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.4 MB/s, tsv 32.0 MB/s, tsv_wasm 20.8 MB/s, oxfmt 20.8 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), biome-wasm 2506/2506 (100%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 109.92     | 547  | 9.03     | 9.35     | 9.65     | 9.92     | 10.38    | 8.55     | 11.93    | baseline                     |
| tsv-json          | 63.55      | 311  | 15.61    | 16.06    | 16.44    | 17.12    | 17.84    | 15.10    | 18.64    | 0.58x                        |
| tsv_wasm-json     | 51.35      | 249  | 19.36    | 19.83    | 20.06    | 20.56    | 21.52    | 18.96    | 21.96    | 0.47x                        |
| tsv-internal      | 307.56     | 1452 | 3.25     | 3.27     | 3.32     | 3.36     | 3.47     | 3.19     | 4.07     | 2.80x                        |
| tsv_wasm-internal | 175.07     | 865  | 5.70     | 5.75     | 5.81     | 5.84     | 5.88     | 5.62     | 6.34     | 1.59x                        |
| postcss           | 99.08      | 488  | 10.04    | 10.28    | 10.61    | 10.81    | 11.35    | 9.68     | 14.31    | 0.90x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 36.8 MB/s, tsv-json 21.3 MB/s, tsv_wasm-json 17.2 MB/s, tsv-internal 102.9 MB/s, tsv_wasm-internal 58.6 MB/s, postcss 33.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.8x tsv-internal, tsv_wasm-json 3.4x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.92       | 10  | 517.13   | 535.75   | 544.76   | 544.90   | 545.02   | 497.63   | 545.04   | baseline              |
| tsv        | 146.17     | 562 | 6.84     | 6.96     | 7.11     | 7.18     | 7.28     | 6.78     | 7.52     | 76.0x                 |
| tsv_wasm   | 83.99      | 322 | 11.88    | 12.15    | 12.25    | 12.29    | 12.38    | 11.81    | 13.29    | 43.7x                 |
| oxfmt      | 59.45      | 295 | 16.77    | 17.16    | 17.57    | 17.78    | 18.32    | 15.64    | 20.48    | 30.9x                 |
| biome-wasm | 9.94       | 42  | 100.39   | 101.84   | 103.18   | 103.38   | 103.51   | 99.62    | 103.63   | 5.17x                 |
| malva-wasm | 20.14      | 85  | 49.47    | 50.38    | 50.66    | 50.87    | 51.07    | 49.18    | 51.47    | 10.5x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 48.9 MB/s, tsv_wasm 28.1 MB/s, oxfmt 19.9 MB/s, biome-wasm 3.3 MB/s, malva-wasm 6.7 MB/s

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
| tsv (ffi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 3.0x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 657.2 KB | 0.4x | 0.4x |
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
| format svelte (773f) | **57.0x** prettier, **57.7x** oxfmt |
| format typescript (2504f) | **22.8x** prettier, **1.54x** oxfmt |
| format css (49f) | **76.0x** prettier, **2.46x** oxfmt |
| parse svelte (773f) | **2.48x** svelte/compiler, **1.81x** rsvelte-parse |
| parse typescript (2503f) | **1.74x** acorn-typescript, **0.66x** oxc-parser, **0.25x** yuku-parser, **0.89x** swc |
| parse css (49f) | **0.58x** svelte/compiler, **0.64x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **36.8x** prettier, **6.38x** biome-wasm |
| format typescript (2504f) | **14.8x** prettier, **4.96x** biome-wasm, **4.20x** dprint-wasm |
| format css (49f) | **43.7x** prettier, **8.45x** biome-wasm, **4.17x** malva-wasm |
| parse svelte (773f) | **2.11x** svelte/compiler |
| parse typescript (2503f) | **1.54x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
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
