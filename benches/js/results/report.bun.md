# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.2

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-15T01:30:23.499Z — tsv 0.3.0 (b2f9c39e)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2, typescript@6.0.3

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.17       | 6   | 851.60   | 862.12   | 879.53   | —        | —        | 840.18   | 895.66   | baseline                     |
| tsv-json                    | 5.90       | 29  | 167.68   | 174.35   | 181.58   | 182.91   | 186.04   | 164.06   | 187.16   | 5.06x                        |
| tsv_wasm-json               | 5.64       | 29  | 175.43   | 180.28   | 188.86   | 191.01   | 192.42   | 170.27   | 192.44   | 4.84x                        |
| tsv-json-no-locations       | 8.26       | 42  | 118.92   | 125.57   | 127.58   | 129.20   | 131.41   | 114.97   | 132.80   | 7.09x                        |
| tsv_wasm-json-no-locations  | 7.44       | 38  | 131.51   | 139.71   | 141.19   | 142.35   | 147.49   | 127.27   | 148.71   | 6.38x                        |
| tsv-internal                | 50.04      | 220 | 19.89    | 20.37    | 20.62    | 20.79    | 21.25    | 19.67    | 21.91    | 42.9x                        |
| tsv_wasm-internal           | 32.64      | 119 | 30.61    | 31.41    | 31.56    | 32.13    | 32.90    | 30.43    | 33.62    | 28.0x                        |
| rsvelte-parse               | 2.02       | 11  | 490.25   | 500.79   | 507.98   | 512.91   | 516.86   | 481.74   | 517.84   | 1.74x                        |
| rsvelte-parse-skip-expr-loc | 2.98       | 15  | 335.68   | 341.56   | 346.36   | 349.58   | 350.70   | 323.96   | 350.98   | 2.55x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 2.8 MB/s, tsv-json 14.2 MB/s, tsv_wasm-json 13.6 MB/s, tsv-json-no-locations 19.9 MB/s, tsv_wasm-json-no-locations 18.0 MB/s, tsv-internal 120.7 MB/s, tsv_wasm-internal 78.7 MB/s, rsvelte-parse 4.9 MB/s, rsvelte-parse-skip-expr-loc 7.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.5x tsv-internal, tsv_wasm-json 5.8x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.22       | 6  | 4.48    | 4.57    | 4.62    | —       | —       | 4.45    | 4.71    | baseline              |
| tsv       | 12.56      | 56 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 56.5x                 |
| tsv_wasm  | 8.70       | 42 | 0.11    | 0.12    | 0.12    | 0.12    | 0.12    | 0.11    | 0.12    | 39.2x                 |
| oxfmt     | 0.21       | 6  | 4.79    | 4.85    | 4.93    | —       | —       | 4.72    | 5.02    | 0.94x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 30.3 MB/s, tsv_wasm 21.0 MB/s, oxfmt 0.5 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.20       | 3  | 5070.11  | 5116.01  | 5119.32  | —        | —        | 5067.43  | 5121.53  | baseline                      |
| tsv-json                   | 0.87       | 5  | 1153.78  | 1158.47  | 1159.18  | —        | —        | 1148.75  | 1159.66  | 4.39x                         |
| tsv_wasm-json              | 0.89       | 5  | 1128.69  | 1129.48  | 1132.51  | —        | —        | 1120.00  | 1134.52  | 4.50x                         |
| tsv-json-no-locations      | 1.52       | 8  | 658.58   | 666.92   | 670.57   | —        | —        | 644.16   | 674.66   | 7.69x                         |
| tsv_wasm-json-no-locations | 1.46       | 8  | 685.08   | 693.05   | 696.03   | —        | —        | 670.47   | 697.13   | 7.40x                         |
| tsv-internal               | 9.94       | 44 | 100.54   | 101.37   | 102.65   | 103.81   | 103.93   | 99.82    | 104.02   | 50.4x                         |
| tsv_wasm-internal          | 6.87       | 28 | 145.50   | 146.53   | 148.35   | 148.79   | 149.40   | 144.70   | 149.49   | 34.8x                         |
| oxc-parser                 | 1.12       | 5  | 889.88   | 890.94   | 895.25   | —        | —        | 887.79   | 899.33   | 5.70x                         |
| oxc-parser-wasm            | 0.84       | 5  | 1201.13  | 1208.57  | 1215.53  | —        | —        | 1155.04  | 1220.16  | 4.26x                         |
| yuku-parser                | 2.81       | 12 | 352.62   | 371.64   | 386.19   | 389.97   | 390.15   | 343.13   | 390.20   | 14.3x                         |
| yuku-parser-wasm           | 3.40       | 17 | 292.58   | 299.05   | 311.41   | 314.27   | 319.09   | 278.47   | 320.30   | 17.2x                         |
| swc                        | 0.72       | 5  | 1384.27  | 1391.60  | 1395.99  | —        | —        | 1375.59  | 1398.91  | 3.66x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 3.6 MB/s, tsv-json 16.0 MB/s, tsv_wasm-json 16.4 MB/s, tsv-json-no-locations 28.0 MB/s, tsv_wasm-json-no-locations 26.9 MB/s, tsv-internal 183.2 MB/s, tsv_wasm-internal 126.8 MB/s, oxc-parser 20.7 MB/s, oxc-parser-wasm 15.5 MB/s, yuku-parser 51.9 MB/s, yuku-parser-wasm 62.7 MB/s, swc 13.3 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.5x tsv-internal, tsv_wasm-json 7.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ----------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier    | 0.07       | 7 | 15336.68 | 15357.70 | 15393.09 | —        | —        | 15260.13 | 15422.66 | baseline              |
| tsv         | 2.24       | 9 | 446.99   | 447.85   | 453.06   | —        | —        | 445.96   | 455.72   | 34.3x                 |
| tsv_wasm    | 1.58       | 6 | 634.57   | 636.25   | 641.16   | —        | —        | 633.84   | 645.29   | 24.2x                 |
| oxfmt       | 1.06       | 6 | 943.29   | 946.60   | 946.91   | —        | —        | 933.67   | 947.04   | 16.3x                 |
| dprint-wasm | 0.31       | 5 | 3211.05  | 3212.58  | 3214.63  | —        | —        | 3209.09  | 3216.00  | 4.77x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.2 MB/s, tsv 41.3 MB/s, tsv_wasm 29.1 MB/s, oxfmt 19.6 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 48.86      | 232 | 20.52    | 20.72    | 21.13    | 21.36    | 23.17    | 19.10    | 23.59    | baseline                     |
| tsv-json          | 57.84      | 290 | 17.13    | 18.24    | 18.63    | 18.79    | 19.39    | 15.89    | 21.16    | 1.18x                        |
| tsv_wasm-json     | 57.54      | 287 | 17.13    | 18.24    | 18.65    | 18.75    | 19.26    | 16.13    | 22.13    | 1.18x                        |
| tsv-internal      | 242.42     | 937 | 4.12     | 4.16     | 4.23     | 4.26     | 4.31     | 4.10     | 4.69     | 4.96x                        |
| tsv_wasm-internal | 151.98     | 749 | 6.56     | 6.63     | 6.75     | 6.79     | 6.91     | 6.48     | 7.09     | 3.11x                        |
| postcss           | 59.61      | 286 | 16.74    | 17.27    | 18.24    | 18.64    | 20.49    | 15.10    | 21.79    | 1.22x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 19.8 MB/s, tsv-json 23.4 MB/s, tsv_wasm-json 23.3 MB/s, tsv-internal 98.2 MB/s, tsv_wasm-internal 61.6 MB/s, postcss 24.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.2x tsv-internal, tsv_wasm-json 2.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.77       | 9   | 568.21   | 572.41   | 579.44   | —        | —        | 541.28   | 582.86   | baseline              |
| tsv        | 126.56     | 593 | 7.88     | 7.98     | 8.08     | 8.15     | 8.25     | 7.68     | 8.44     | 71.5x                 |
| tsv_wasm   | 88.22      | 427 | 11.30    | 11.42    | 11.53    | 11.57    | 11.75    | 11.16    | 12.10    | 49.9x                 |
| oxfmt      | 49.42      | 243 | 20.19    | 20.70    | 21.30    | 21.84    | 23.00    | 18.73    | 25.09    | 27.9x                 |
| malva-wasm | 16.30      | 72  | 61.23    | 62.10    | 63.52    | 63.77    | 64.07    | 60.70    | 64.47    | 9.22x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.7 MB/s, tsv 51.3 MB/s, tsv_wasm 35.7 MB/s, oxfmt 20.0 MB/s, malva-wasm 6.6 MB/s

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

_`vs tsv` divides native rows by `tsv (napi)` — the binding this runtime benchmarks (FFI under Deno, N-API under Node/Bun), so the same artifact reads a different ratio in the deno and node/bun reports — and wasm rows by `tsv_wasm`. Gzipped ≈ the artifact’s wire size (`gzip -c`, system default level; the `tsv (napi)` platform package also ships the `tsv` CLI binary, so its tarball is larger than this row). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **56.5x** prettier, **60.1x** oxfmt |
| format typescript (2643f) | **34.3x** prettier, **2.11x** oxfmt |
| format css (55f) | **71.5x** prettier, **2.56x** oxfmt |
| parse svelte (951f) | **5.06x** svelte/compiler, **2.91x** rsvelte-parse |
| parse typescript (2642f) | **4.39x** acorn-typescript, **0.77x** oxc-parser, **0.31x** yuku-parser, **1.20x** swc |
| parse css (55f) | **1.18x** svelte/compiler, **0.97x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **39.2x** prettier |
| format typescript (2643f) | **24.2x** prettier, **5.06x** dprint-wasm |
| format css (55f) | **49.9x** prettier, **5.41x** malva-wasm |
| parse svelte (951f) | **4.84x** svelte/compiler |
| parse typescript (2642f) | **4.50x** acorn-typescript, **1.06x** oxc-parser-wasm, **0.26x** yuku-parser-wasm |
| parse css (55f) | **1.18x** svelte/compiler, **0.97x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire; measured on the perf corpus its JSON is 0.64× `tsv-json`’s bytes. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (0.13% smaller in aggregate at the current pin, per-component median exactly 1.00 — the axis a throughput ratio integrates), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. Not payload-matched either: it keeps selectors and values as strings where `parseCss` (and so tsv) builds full ASTs — 0.38× tsv’s node count and at most 0.56× its JSON bytes on the perf corpus. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

5 timed row(s) were not stable: a cv past 10% (std_dev / mean — `cv` after outlier removal; `cv (raw)` before it, which counts only under 30 raw samples, where one deviant sweep is a real share of the row) or a drift past 5% (the median of the second half of the timings against the first's — a cost that moved WHILE the row was measured, which the cleaned cv cannot see: a second mode is deleted or blended, not reported). Every `Nx` involving one of these divides a mean that may be neither mode — read it as approximate, and re-run before drawing a conclusion from it; a longer window does not converge a drifting row, it moves the answer.

| Row | cv | cv (raw) | drift | samples (cleaned/raw) |
| --- | ---: | ---: | ---: | ---: |
| parse/css/tsv-json | 6.1% | 6.1% | -10.6% | 290/290 |
| parse/css/tsv_wasm-json | 5.4% | 5.6% | -9.9% | 287/288 |
| parse/svelte/tsv_wasm-json-no-locations | 4.4% | 4.4% | -7.5% | 38/38 |
| parse/svelte/tsv-json-no-locations | 4.1% | 4.1% | -6.9% | 42/42 |
| parse/svelte/tsv-json | 3.6% | 4.0% | -5.5% | 29/30 |

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
