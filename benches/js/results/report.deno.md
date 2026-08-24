# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.5

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T22:05:43.811Z — tsv 0.2.0 (3d0eea49)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.06       | 11  | 477.92   | 491.58   | 509.00   | 512.61   | 515.50   | 469.01   | 516.22   | baseline                     |
| tsv-json                    | 5.18       | 21  | 193.11   | 194.99   | 199.29   | 200.28   | 200.86   | 191.43   | 200.95   | 2.51x                        |
| tsv_wasm-json               | 4.36       | 21  | 228.60   | 230.73   | 235.16   | 235.64   | 236.62   | 225.91   | 236.88   | 2.11x                        |
| tsv-json-no-locations       | 8.08       | 41  | 122.94   | 125.48   | 127.75   | 128.02   | 128.93   | 121.30   | 129.13   | 3.91x                        |
| tsv_wasm-json-no-locations  | 6.59       | 29  | 151.41   | 153.67   | 156.76   | 157.03   | 157.16   | 150.17   | 157.18   | 3.19x                        |
| tsv-internal                | 49.50      | 189 | 20.12    | 20.80    | 21.22    | 21.82    | 22.16    | 19.98    | 22.48    | 24.0x                        |
| tsv_wasm-internal           | 31.89      | 146 | 31.11    | 32.00    | 32.19    | 32.30    | 32.47    | 30.93    | 32.78    | 15.4x                        |
| rsvelte-parse               | 2.85       | 13  | 349.65   | 354.72   | 360.61   | 364.25   | 368.17   | 347.21   | 369.14   | 1.38x                        |
| rsvelte-parse-skip-expr-loc | 4.79       | 24  | 207.24   | 210.14   | 214.47   | 215.42   | 215.80   | 204.97   | 215.88   | 2.32x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 10.2 MB/s, tsv_wasm-json 8.6 MB/s, tsv-json-no-locations 15.9 MB/s, tsv_wasm-json-no-locations 12.9 MB/s, tsv-internal 97.1 MB/s, tsv_wasm-internal 62.5 MB/s, rsvelte-parse 5.6 MB/s, rsvelte-parse-skip-expr-loc 9.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.6x tsv-internal, tsv_wasm-json 7.3x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 6  | 4364.57  | 4419.15  | 4553.14  | —        | —        | 4273.79  | 4739.74  | baseline              |
| tsv        | 12.95      | 65 | 76.85    | 78.38    | 80.09    | 80.44    | 80.76    | 75.42    | 80.90    | 56.3x                 |
| tsv_wasm   | 8.35       | 42 | 119.06   | 120.76   | 123.11   | 123.54   | 124.23   | 117.81   | 124.29   | 36.3x                 |
| oxfmt      | 0.23       | 6  | 4374.11  | 4440.39  | 4555.56  | —        | —        | 4325.71  | 4666.67  | 0.99x                 |
| biome-wasm | 1.32       | 6  | 759.26   | 761.98   | 771.03   | —        | —        | 756.58   | 781.29   | 5.73x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.5 MB/s, tsv 25.4 MB/s, tsv_wasm 16.4 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 4  | 3.18    | 3.18    | 3.19    | —       | —       | 3.17    | 3.20    | baseline                      |
| tsv-json                   | 0.54       | 4  | 1.85    | 1.86    | 1.86    | —       | —       | 1.85    | 1.87    | 1.72x                         |
| tsv_wasm-json              | 0.48       | 5  | 2.08    | 2.08    | 2.09    | —       | —       | 2.07    | 2.10    | 1.53x                         |
| tsv-json-no-locations      | 1.10       | 6  | 0.91    | 0.91    | 0.92    | —       | —       | 0.91    | 0.92    | 3.49x                         |
| tsv_wasm-json-no-locations | 0.92       | 5  | 1.08    | 1.08    | 1.08    | —       | —       | 1.08    | 1.08    | 2.94x                         |
| tsv-internal               | 7.39       | 34 | 0.13    | 0.14    | 0.14    | 0.14    | 0.14    | 0.13    | 0.14    | 23.5x                         |
| tsv_wasm-internal          | 5.07       | 21 | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 16.1x                         |
| oxc-parser                 | 0.82       | 5  | 1.22    | 1.24    | 1.24    | —       | —       | 1.18    | 1.25    | 2.61x                         |
| oxc-parser-wasm            | 0.73       | 3  | 1.36    | 1.36    | 1.37    | —       | —       | 1.36    | 1.37    | 2.34x                         |
| yuku-parser                | 2.20       | 11 | 0.46    | 0.46    | 0.47    | 0.47    | 0.48    | 0.42    | 0.48    | 6.98x                         |
| yuku-parser-wasm           | 2.45       | 11 | 0.41    | 0.42    | 0.43    | 0.47    | 0.51    | 0.39    | 0.52    | 7.80x                         |
| swc                        | 0.61       | 5  | 1.63    | 1.63    | 1.64    | —       | —       | 1.63    | 1.64    | 1.95x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 5.7 MB/s, tsv-json 9.7 MB/s, tsv_wasm-json 8.6 MB/s, tsv-json-no-locations 19.7 MB/s, tsv_wasm-json-no-locations 16.6 MB/s, tsv-internal 132.8 MB/s, tsv_wasm-internal 91.1 MB/s, oxc-parser 14.8 MB/s, oxc-parser-wasm 13.2 MB/s, yuku-parser 39.5 MB/s, yuku-parser-wasm 44.1 MB/s, swc 11.0 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 13.7x tsv-internal, tsv_wasm-json 10.5x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 7 | 12.99   | 13.07   | 13.17   | —       | —       | 12.87   | 13.24   | baseline              |
| tsv         | 1.75       | 7 | 0.57    | 0.58    | 0.59    | —       | —       | 0.57    | 0.59    | 22.8x                 |
| tsv_wasm    | 1.16       | 5 | 0.86    | 0.87    | 0.87    | —       | —       | 0.86    | 0.88    | 15.0x                 |
| oxfmt       | 1.14       | 6 | 0.88    | 0.88    | 0.89    | —       | —       | 0.86    | 0.89    | 14.8x                 |
| biome-wasm  | 0.23       | 5 | 4.33    | 4.33    | 4.33    | —       | —       | 4.32    | 4.34    | 3.01x                 |
| dprint-wasm | 0.28       | 5 | 3.61    | 3.62    | 3.62    | —       | —       | 3.61    | 3.62    | 3.60x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.4 MB/s, tsv 31.5 MB/s, tsv_wasm 20.8 MB/s, oxfmt 20.5 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), biome-wasm 2506/2506 (100%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 109.46     | 545  | 9.09     | 9.38     | 9.67     | 9.82     | 10.26    | 8.59     | 12.33    | baseline                     |
| tsv-json          | 64.40      | 311  | 15.42    | 15.90    | 16.15    | 16.69    | 17.59    | 15.07    | 17.86    | 0.59x                        |
| tsv_wasm-json     | 51.22      | 249  | 19.41    | 19.88    | 20.11    | 20.59    | 21.57    | 18.99    | 21.84    | 0.47x                        |
| tsv-internal      | 317.97     | 1336 | 3.14     | 3.18     | 3.24     | 3.26     | 3.29     | 3.12     | 3.58     | 2.90x                        |
| tsv_wasm-internal | 173.82     | 739  | 5.74     | 5.81     | 5.88     | 5.90     | 5.95     | 5.70     | 7.30     | 1.59x                        |
| postcss           | 98.61      | 484  | 10.06    | 10.35    | 10.70    | 11.02    | 11.58    | 9.67     | 13.16    | 0.90x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 36.6 MB/s, tsv-json 21.6 MB/s, tsv_wasm-json 17.1 MB/s, tsv-internal 106.4 MB/s, tsv_wasm-internal 58.2 MB/s, postcss 33.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.9x tsv-internal, tsv_wasm-json 3.4x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.95       | 10  | 511.48   | 522.75   | 532.62   | 533.38   | 533.99   | 493.07   | 534.15   | baseline              |
| tsv        | 151.10     | 684 | 6.59     | 6.71     | 6.85     | 6.93     | 7.03     | 6.51     | 7.99     | 77.3x                 |
| tsv_wasm   | 84.72      | 424 | 11.78    | 11.93    | 12.01    | 12.04    | 12.13    | 11.61    | 12.29    | 43.4x                 |
| oxfmt      | 59.17      | 294 | 16.86    | 17.24    | 17.55    | 17.88    | 18.34    | 15.77    | 21.26    | 30.3x                 |
| biome-wasm | 9.81       | 49  | 101.34   | 103.39   | 104.49   | 104.74   | 105.05   | 100.26   | 105.21   | 5.02x                 |
| malva-wasm | 20.15      | 83  | 49.50    | 50.34    | 51.06    | 51.36    | 53.63    | 49.19    | 54.10    | 10.3x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.7 MB/s, tsv 50.6 MB/s, tsv_wasm 28.4 MB/s, oxfmt 19.8 MB/s, biome-wasm 3.3 MB/s, malva-wasm 6.7 MB/s

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
| format svelte (773f) | **56.3x** prettier, **56.7x** oxfmt |
| format typescript (2504f) | **22.8x** prettier, **1.54x** oxfmt |
| format css (49f) | **77.3x** prettier, **2.55x** oxfmt |
| parse svelte (773f) | **2.51x** svelte/compiler, **1.82x** rsvelte-parse |
| parse typescript (2503f) | **1.72x** acorn-typescript, **0.66x** oxc-parser, **0.25x** yuku-parser, **0.88x** swc |
| parse css (49f) | **0.59x** svelte/compiler, **0.65x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **36.3x** prettier, **6.34x** biome-wasm |
| format typescript (2504f) | **15.0x** prettier, **5.00x** biome-wasm, **4.18x** dprint-wasm |
| format css (49f) | **43.4x** prettier, **8.64x** biome-wasm, **4.20x** malva-wasm |
| parse svelte (773f) | **2.11x** svelte/compiler |
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
