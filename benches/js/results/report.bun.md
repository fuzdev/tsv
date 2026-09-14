# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-14T22:31:51.671Z — tsv 0.3.0 (7eb42464)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.18       | 6   | 842.04   | 852.21   | 869.29   | —        | —        | 832.24   | 884.38   | baseline                     |
| tsv-json                    | 5.99       | 26  | 165.87   | 171.83   | 179.21   | 180.98   | 184.95   | 162.42   | 186.24   | 5.08x                        |
| tsv_wasm-json               | 5.68       | 27  | 174.35   | 179.31   | 187.72   | 190.20   | 193.16   | 171.08   | 193.77   | 4.82x                        |
| tsv-json-no-locations       | 8.27       | 42  | 118.80   | 125.48   | 128.00   | 129.53   | 130.47   | 115.12   | 130.62   | 7.02x                        |
| tsv_wasm-json-no-locations  | 7.46       | 38  | 131.39   | 139.33   | 141.92   | 143.83   | 145.98   | 126.72   | 146.88   | 6.33x                        |
| tsv-internal                | 50.74      | 177 | 19.71    | 20.25    | 20.41    | 20.55    | 20.74    | 19.58    | 21.63    | 43.0x                        |
| tsv_wasm-internal           | 32.54      | 140 | 30.61    | 31.36    | 31.65    | 32.08    | 32.40    | 30.30    | 32.74    | 27.6x                        |
| rsvelte-parse               | 2.03       | 10  | 488.95   | 500.25   | 509.35   | 514.02   | 517.76   | 483.25   | 518.69   | 1.72x                        |
| rsvelte-parse-skip-expr-loc | 2.97       | 15  | 335.22   | 341.00   | 349.00   | 350.64   | 352.08   | 326.21   | 352.44   | 2.52x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 2.8 MB/s, tsv-json 14.4 MB/s, tsv_wasm-json 13.7 MB/s, tsv-json-no-locations 20.0 MB/s, tsv_wasm-json-no-locations 18.0 MB/s, tsv-internal 122.4 MB/s, tsv_wasm-internal 78.5 MB/s, rsvelte-parse 4.9 MB/s, rsvelte-parse-skip-expr-loc 7.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.5x tsv-internal, tsv_wasm-json 5.7x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.21       | 6  | 4.63    | 4.79    | 4.87    | —       | —       | 4.57    | 4.96    | baseline              |
| tsv       | 12.59      | 58 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 58.7x                 |
| tsv_wasm  | 8.74       | 44 | 0.11    | 0.12    | 0.12    | 0.12    | 0.12    | 0.11    | 0.12    | 40.7x                 |
| oxfmt     | 0.21       | 7  | 4.87    | 4.93    | 4.95    | —       | —       | 4.69    | 4.97    | 0.96x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 30.4 MB/s, tsv_wasm 21.1 MB/s, oxfmt 0.5 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.20       | 5  | 4908.87  | 4939.80  | 4951.54  | —        | —        | 4881.33  | 4959.37  | baseline                      |
| tsv-json                   | 0.87       | 4  | 1148.97  | 1152.49  | 1159.35  | —        | —        | 1146.44  | 1163.92  | 4.28x                         |
| tsv_wasm-json              | 0.89       | 5  | 1123.87  | 1125.02  | 1129.69  | —        | —        | 1118.79  | 1132.81  | 4.37x                         |
| tsv-json-no-locations      | 1.53       | 8  | 653.96   | 661.15   | 663.04   | —        | —        | 642.25   | 663.41   | 7.52x                         |
| tsv_wasm-json-no-locations | 1.46       | 8  | 685.24   | 692.42   | 694.75   | —        | —        | 673.01   | 699.27   | 7.17x                         |
| tsv-internal               | 10.04      | 46 | 99.37    | 100.35   | 100.92   | 101.16   | 102.40   | 98.90    | 102.68   | 49.4x                         |
| tsv_wasm-internal          | 6.97       | 32 | 143.20   | 144.13   | 146.68   | 147.40   | 147.88   | 141.67   | 147.96   | 34.3x                         |
| oxc-parser                 | 1.13       | 6  | 887.63   | 892.31   | 893.85   | —        | —        | 874.80   | 893.90   | 5.54x                         |
| oxc-parser-wasm            | 0.87       | 5  | 1154.67  | 1158.06  | 1177.90  | —        | —        | 1111.86  | 1191.12  | 4.27x                         |
| yuku-parser                | 2.84       | 15 | 346.23   | 362.71   | 376.20   | 381.85   | 382.53   | 334.64   | 382.70   | 13.9x                         |
| yuku-parser-wasm           | 3.45       | 18 | 287.19   | 295.06   | 306.56   | 313.29   | 323.61   | 273.98   | 326.19   | 16.9x                         |
| swc                        | 0.74       | 4  | 1350.73  | 1362.75  | 1367.67  | —        | —        | 1349.05  | 1370.95  | 3.63x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.7 MB/s, tsv-json 16.1 MB/s, tsv_wasm-json 16.4 MB/s, tsv-json-no-locations 28.2 MB/s, tsv_wasm-json-no-locations 26.9 MB/s, tsv-internal 185.2 MB/s, tsv_wasm-internal 128.6 MB/s, oxc-parser 20.8 MB/s, oxc-parser-wasm 16.0 MB/s, yuku-parser 52.3 MB/s, yuku-parser-wasm 63.5 MB/s, swc 13.6 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv_wasm-json 7.8x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ----------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier    | 0.07       | 7 | 15299.34 | 15357.63 | 15397.91 | —        | —        | 15239.94 | 15424.02 | baseline              |
| tsv         | 2.25       | 9 | 443.56   | 444.43   | 452.04   | —        | —        | 442.69   | 453.36   | 34.5x                 |
| tsv_wasm    | 1.58       | 6 | 631.56   | 633.77   | 639.16   | —        | —        | 629.34   | 641.61   | 24.3x                 |
| oxfmt       | 1.08       | 6 | 926.59   | 929.32   | 930.79   | —        | —        | 911.80   | 931.36   | 16.6x                 |
| dprint-wasm | 0.31       | 5 | 3177.06  | 3179.35  | 3181.26  | —        | —        | 3175.23  | 3182.53  | 4.82x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.2 MB/s, tsv 41.6 MB/s, tsv_wasm 29.2 MB/s, oxfmt 20.0 MB/s, dprint-wasm 5.8 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 48.26      | 235  | 20.77    | 21.24    | 21.55    | 21.79    | 24.20    | 18.68    | 25.93    | baseline                     |
| tsv-json          | 57.42      | 288  | 17.26    | 18.25    | 18.70    | 19.05    | 19.55    | 16.08    | 21.26    | 1.19x                        |
| tsv_wasm-json     | 57.32      | 286  | 17.19    | 18.34    | 18.68    | 18.81    | 18.97    | 16.31    | 21.99    | 1.19x                        |
| tsv-internal      | 240.06     | 1015 | 4.16     | 4.20     | 4.26     | 4.29     | 4.33     | 4.13     | 4.38     | 4.97x                        |
| tsv_wasm-internal | 154.29     | 627  | 6.47     | 6.55     | 6.64     | 6.66     | 6.70     | 6.43     | 6.81     | 3.20x                        |
| postcss           | 62.38      | 260  | 15.93    | 16.69    | 17.41    | 17.85    | 20.47    | 14.81    | 22.80    | 1.29x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.6 MB/s, tsv-json 23.3 MB/s, tsv_wasm-json 23.2 MB/s, tsv-internal 97.2 MB/s, tsv_wasm-internal 62.5 MB/s, postcss 25.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.2x tsv-internal, tsv_wasm-json 2.7x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.84       | 10  | 535.82   | 552.68   | 565.57   | 569.68   | 572.97   | 527.00   | 573.79   | baseline              |
| tsv        | 126.62     | 602 | 7.87     | 7.97     | 8.07     | 8.12     | 8.21     | 7.74     | 8.51     | 68.7x                 |
| tsv_wasm   | 87.39      | 433 | 11.40    | 11.53    | 11.62    | 11.66    | 11.75    | 11.27    | 11.95    | 47.4x                 |
| oxfmt      | 48.41      | 237 | 20.64    | 21.16    | 21.69    | 22.18    | 23.60    | 18.93    | 24.55    | 26.3x                 |
| malva-wasm | 16.31      | 76  | 61.13    | 61.73    | 62.45    | 62.73    | 63.55    | 60.71    | 65.97    | 8.85x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.7 MB/s, tsv 51.3 MB/s, tsv_wasm 35.4 MB/s, oxfmt 19.6 MB/s, malva-wasm 6.6 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.5 MB | 928.4 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 995.6 KB | 384.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.8 MB | 1.0 MB | — | — |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.6 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.6 MB | 684.0 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.6x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.5x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.3x | 2.1x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.3x | 2.0x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 4.5x | 4.2x |
| swc (napi) | 32.7 MB | 12.2 MB | 8.4x | 6.9x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **58.7x** prettier, **61.2x** oxfmt |
| format typescript (2643f) | **34.5x** prettier, **2.08x** oxfmt |
| format css (55f) | **68.7x** prettier, **2.62x** oxfmt |
| parse svelte (951f) | **5.08x** svelte/compiler, **2.95x** rsvelte-parse |
| parse typescript (2642f) | **4.28x** acorn-typescript, **0.77x** oxc-parser, **0.31x** yuku-parser, **1.18x** swc |
| parse css (55f) | **1.19x** svelte/compiler, **0.92x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **40.7x** prettier |
| format typescript (2643f) | **24.3x** prettier, **5.04x** dprint-wasm |
| format css (55f) | **47.4x** prettier, **5.36x** malva-wasm |
| parse svelte (951f) | **4.82x** svelte/compiler |
| parse typescript (2642f) | **4.37x** acorn-typescript, **1.02x** oxc-parser-wasm, **0.26x** yuku-parser-wasm |
| parse css (55f) | **1.19x** svelte/compiler, **0.92x** postcss |

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
