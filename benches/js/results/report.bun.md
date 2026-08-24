# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.0

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T01:25:14.821Z — tsv 0.2.0 (5950c4ad)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.57       | 7   | 635.89   | 644.98   | 670.92   | —        | —        | 624.22   | 691.54   | baseline                     |
| tsv-json                    | 6.94       | 30  | 143.08   | 147.71   | 161.64   | 172.98   | 174.86   | 139.11   | 175.58   | 4.42x                        |
| tsv_wasm-json               | 6.68       | 29  | 148.08   | 153.34   | 165.91   | 177.59   | 179.10   | 144.88   | 179.33   | 4.25x                        |
| tsv-json-no-locations       | 9.62       | 47  | 103.02   | 108.31   | 111.80   | 117.24   | 120.88   | 98.24    | 123.29   | 6.12x                        |
| tsv_wasm-json-no-locations  | 8.98       | 40  | 109.48   | 117.29   | 122.22   | 127.29   | 130.75   | 107.01   | 131.32   | 5.71x                        |
| tsv-internal                | 53.63      | 178 | 18.63    | 19.40    | 19.52    | 19.59    | 19.72    | 18.52    | 20.05    | 34.1x                        |
| tsv_wasm-internal           | 36.72      | 127 | 27.21    | 28.16    | 28.36    | 28.50    | 28.69    | 27.04    | 29.00    | 23.4x                        |
| rsvelte-parse               | 3.27       | 13  | 305.16   | 317.76   | 337.55   | 343.80   | 346.97   | 298.74   | 347.77   | 2.08x                        |
| rsvelte-parse-skip-expr-loc | 5.12       | 26  | 194.20   | 201.85   | 204.28   | 205.65   | 206.27   | 185.78   | 206.46   | 3.26x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 3.1 MB/s, tsv-json 13.6 MB/s, tsv_wasm-json 13.1 MB/s, tsv-json-no-locations 18.9 MB/s, tsv_wasm-json-no-locations 17.6 MB/s, tsv-internal 105.2 MB/s, tsv_wasm-internal 72.0 MB/s, rsvelte-parse 6.4 MB/s, rsvelte-parse-skip-expr-loc 10.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.7x tsv-internal, tsv_wasm-json 5.5x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.29       | 4  | 3.44    | 3.44    | 3.57    | —       | —       | 3.42    | 3.65    | baseline              |
| tsv       | 12.99      | 65 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.07    | 0.08    | 44.6x                 |
| tsv_wasm  | 9.38       | 47 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 32.2x                 |
| oxfmt     | 0.27       | 5  | 3.72    | 3.76    | 3.77    | —       | —       | 3.61    | 3.78    | 0.93x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.6 MB/s, tsv 25.5 MB/s, tsv_wasm 18.4 MB/s, oxfmt 0.5 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.20       | 5  | 4962.33  | 4966.72  | 4976.50  | —        | —        | 4946.50  | 4983.02  | baseline                      |
| tsv-json                   | 0.84       | 5  | 1195.19  | 1195.86  | 1197.14  | —        | —        | 1187.01  | 1197.99  | 4.16x                         |
| tsv_wasm-json              | 0.86       | 5  | 1168.67  | 1169.61  | 1170.57  | —        | —        | 1165.36  | 1171.21  | 4.25x                         |
| tsv-json-no-locations      | 1.45       | 8  | 689.46   | 696.78   | 704.02   | —        | —        | 679.43   | 705.96   | 7.18x                         |
| tsv_wasm-json-no-locations | 1.38       | 7  | 723.36   | 726.30   | 732.92   | —        | —        | 712.98   | 739.47   | 6.86x                         |
| tsv-internal               | 8.15       | 37 | 122.56   | 123.28   | 125.37   | 125.68   | 125.94   | 121.77   | 125.95   | 40.4x                         |
| tsv_wasm-internal          | 5.83       | 23 | 171.28   | 172.65   | 175.92   | 176.21   | 176.53   | 170.77   | 176.64   | 29.0x                         |
| oxc-parser                 | 1.14       | 6  | 871.32   | 878.06   | 882.09   | —        | —        | 866.69   | 884.49   | 5.68x                         |
| oxc-parser-wasm            | 0.83       | 4  | 1209.50  | 1210.56  | 1240.55  | —        | —        | 1202.94  | 1260.54  | 4.11x                         |
| yuku-parser                | 3.02       | 13 | 329.97   | 343.01   | 373.76   | 384.91   | 388.13   | 319.81   | 388.93   | 15.0x                         |
| yuku-parser-wasm           | 3.59       | 18 | 273.78   | 286.17   | 294.99   | 302.51   | 319.67   | 262.46   | 323.96   | 17.8x                         |
| swc                        | 0.74       | 5  | 1345.91  | 1346.83  | 1347.88  | —        | —        | 1341.24  | 1348.59  | 3.69x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 3.6 MB/s, tsv-json 15.1 MB/s, tsv_wasm-json 15.4 MB/s, tsv-json-no-locations 26.0 MB/s, tsv_wasm-json-no-locations 24.8 MB/s, tsv-internal 146.5 MB/s, tsv_wasm-internal 104.9 MB/s, oxc-parser 20.6 MB/s, oxc-parser-wasm 14.9 MB/s, yuku-parser 54.2 MB/s, yuku-parser-wasm 64.6 MB/s, swc 13.4 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.7x tsv-internal, tsv_wasm-json 6.8x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ----------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier    | 0.07       | 7 | 13556.99 | 13621.99 | 13685.53 | —        | —        | 13237.06 | 13716.13 | baseline              |
| tsv         | 1.73       | 7 | 578.45   | 579.32   | 587.26   | —        | —        | 577.49   | 587.64   | 23.3x                 |
| tsv_wasm    | 1.31       | 6 | 766.46   | 767.05   | 771.96   | —        | —        | 763.97   | 778.48   | 17.6x                 |
| oxfmt       | 1.15       | 6 | 869.73   | 871.23   | 872.93   | —        | —        | 858.33   | 874.58   | 15.5x                 |
| dprint-wasm | 0.32       | 5 | 3145.20  | 3145.84  | 3148.01  | —        | —        | 3143.02  | 3149.45  | 4.28x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.3 MB/s, tsv 31.1 MB/s, tsv_wasm 23.5 MB/s, oxfmt 20.7 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 65.04      | 240  | 15.43    | 15.88    | 22.94    | 23.37    | 23.73    | 14.73    | 24.15    | baseline                     |
| tsv-json          | 72.34      | 361  | 13.80    | 14.40    | 14.88    | 15.14    | 15.73    | 12.80    | 18.33    | 1.11x                        |
| tsv_wasm-json     | 74.67      | 371  | 13.19    | 13.98    | 14.40    | 14.54    | 15.16    | 12.55    | 18.55    | 1.15x                        |
| tsv-internal      | 324.76     | 1256 | 3.08     | 3.10     | 3.17     | 3.23     | 3.32     | 3.05     | 3.99     | 4.99x                        |
| tsv_wasm-internal | 224.13     | 885  | 4.46     | 4.51     | 4.57     | 4.60     | 4.63     | 4.43     | 4.86     | 3.45x                        |
| postcss           | 84.83      | 345  | 11.80    | 11.97    | 12.63    | 19.17    | 19.62    | 11.21    | 22.08    | 1.30x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 21.8 MB/s, tsv-json 24.2 MB/s, tsv_wasm-json 25.0 MB/s, tsv-internal 108.7 MB/s, tsv_wasm-internal 75.0 MB/s, postcss 28.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.5x tsv-internal, tsv_wasm-json 3.0x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.51       | 11  | 400.21   | 405.48   | 445.64   | 469.62   | 487.58   | 388.96   | 492.07   | baseline              |
| tsv        | 140.80     | 606 | 7.09     | 7.18     | 7.38     | 7.50     | 7.73     | 7.01     | 8.22     | 56.1x                 |
| tsv_wasm   | 101.04     | 440 | 9.87     | 10.01    | 10.25    | 10.39    | 10.59    | 9.75     | 11.05    | 40.3x                 |
| oxfmt      | 58.12      | 283 | 17.15    | 17.58    | 18.13    | 18.55    | 21.25    | 15.53    | 22.07    | 23.2x                 |
| malva-wasm | 18.11      | 85  | 54.92    | 55.97    | 56.42    | 56.65    | 57.46    | 54.48    | 58.13    | 7.22x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.8 MB/s, tsv 47.1 MB/s, tsv_wasm 33.8 MB/s, oxfmt 19.4 MB/s, malva-wasm 6.1 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 838.1 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 928.7 KB | 361.5 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 930.5 KB | — | — |
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
| format svelte (773f) | **44.6x** prettier, **48.1x** oxfmt |
| format typescript (2504f) | **23.3x** prettier, **1.50x** oxfmt |
| format css (49f) | **56.1x** prettier, **2.42x** oxfmt |
| parse svelte (773f) | **4.42x** svelte/compiler, **2.12x** rsvelte-parse |
| parse typescript (2503f) | **4.16x** acorn-typescript, **0.73x** oxc-parser, **0.28x** yuku-parser, **1.13x** swc |
| parse css (49f) | **1.11x** svelte/compiler, **0.85x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **32.2x** prettier |
| format typescript (2504f) | **17.6x** prettier, **4.11x** dprint-wasm |
| format css (49f) | **40.3x** prettier, **5.58x** malva-wasm |
| parse svelte (773f) | **4.25x** svelte/compiler |
| parse typescript (2503f) | **4.25x** acorn-typescript, **1.03x** oxc-parser-wasm, **0.24x** yuku-parser-wasm |
| parse css (49f) | **1.15x** svelte/compiler, **0.88x** postcss |

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
