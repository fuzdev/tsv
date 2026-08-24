# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.4.0

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T22:25:06.303Z — tsv 0.2.0 (3d0eea49)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.59       | 6   | 628.47   | 656.13   | 692.94   | —        | —        | 615.98   | 711.47   | baseline                     |
| tsv-json                    | 6.90       | 30  | 144.18   | 148.52   | 163.49   | 175.26   | 175.72   | 140.05   | 175.91   | 4.33x                        |
| tsv_wasm-json               | 6.68       | 29  | 148.69   | 152.26   | 172.32   | 181.77   | 182.97   | 144.59   | 183.03   | 4.19x                        |
| tsv-json-no-locations       | 9.83       | 44  | 100.65   | 107.46   | 111.31   | 118.25   | 121.89   | 97.03    | 124.56   | 6.17x                        |
| tsv_wasm-json-no-locations  | 8.85       | 40  | 111.68   | 118.10   | 125.93   | 131.55   | 133.68   | 108.08   | 134.43   | 5.55x                        |
| tsv-internal                | 52.94      | 257 | 18.74    | 19.31    | 19.50    | 19.63    | 19.83    | 18.47    | 20.69    | 33.2x                        |
| tsv_wasm-internal           | 35.91      | 134 | 27.78    | 28.67    | 28.95    | 29.03    | 29.37    | 27.56    | 29.51    | 22.5x                        |
| rsvelte-parse               | 3.23       | 13  | 309.05   | 323.93   | 342.76   | 346.56   | 349.86   | 303.04   | 350.68   | 2.02x                        |
| rsvelte-parse-skip-expr-loc | 5.13       | 26  | 194.48   | 203.62   | 204.68   | 204.96   | 207.14   | 184.04   | 207.85   | 3.22x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 3.1 MB/s, tsv-json 13.5 MB/s, tsv_wasm-json 13.1 MB/s, tsv-json-no-locations 19.3 MB/s, tsv_wasm-json-no-locations 17.3 MB/s, tsv-internal 103.8 MB/s, tsv_wasm-internal 70.4 MB/s, rsvelte-parse 6.3 MB/s, rsvelte-parse-skip-expr-loc 10.1 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 7.7x tsv-internal, tsv_wasm-json 5.4x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.29       | 4  | 3.46    | 3.47    | 3.61    | —       | —       | 3.44    | 3.70    | baseline              |
| tsv       | 13.10      | 66 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.07    | 0.08    | 45.3x                 |
| tsv_wasm  | 9.35       | 46 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.10    | 0.11    | 32.3x                 |
| oxfmt     | 0.26       | 5  | 3.80    | 3.85    | 3.92    | —       | —       | 3.65    | 3.97    | 0.91x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.6 MB/s, tsv 25.7 MB/s, tsv_wasm 18.3 MB/s, oxfmt 0.5 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.20       | 4  | 5044.53  | 5048.01  | 5052.32  | —        | —        | 5042.15  | 5055.19  | baseline                      |
| tsv-json                   | 0.84       | 5  | 1188.83  | 1190.45  | 1192.34  | —        | —        | 1185.88  | 1193.60  | 4.24x                         |
| tsv_wasm-json              | 0.86       | 5  | 1167.26  | 1168.67  | 1168.73  | —        | —        | 1160.26  | 1168.77  | 4.33x                         |
| tsv-json-no-locations      | 1.46       | 8  | 683.55   | 689.53   | 697.29   | —        | —        | 674.79   | 699.74   | 7.36x                         |
| tsv_wasm-json-no-locations | 1.37       | 7  | 726.80   | 735.15   | 738.76   | —        | —        | 716.68   | 740.25   | 6.93x                         |
| tsv-internal               | 8.11       | 35 | 123.28   | 124.08   | 126.65   | 126.86   | 127.93   | 122.20   | 128.03   | 40.9x                         |
| tsv_wasm-internal          | 5.74       | 24 | 174.08   | 177.14   | 179.05   | 179.41   | 179.58   | 173.14   | 179.63   | 29.0x                         |
| oxc-parser                 | 1.14       | 6  | 878.02   | 883.47   | 888.64   | —        | —        | 873.74   | 892.01   | 5.73x                         |
| oxc-parser-wasm            | 0.83       | 5  | 1211.08  | 1219.41  | 1247.21  | —        | —        | 1164.07  | 1265.74  | 4.17x                         |
| yuku-parser                | 2.89       | 14 | 342.72   | 353.71   | 385.03   | 397.51   | 403.83   | 329.68   | 405.41   | 14.6x                         |
| yuku-parser-wasm           | 3.77       | 18 | 262.27   | 274.50   | 283.48   | 292.41   | 313.06   | 252.13   | 318.22   | 19.1x                         |
| swc                        | 0.75       | 4  | 1342.04  | 1342.99  | 1343.34  | —        | —        | 1338.95  | 1343.57  | 3.76x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 3.6 MB/s, tsv-json 15.1 MB/s, tsv_wasm-json 15.4 MB/s, tsv-json-no-locations 26.2 MB/s, tsv_wasm-json-no-locations 24.7 MB/s, tsv-internal 145.7 MB/s, tsv_wasm-internal 103.2 MB/s, oxc-parser 20.4 MB/s, oxc-parser-wasm 14.8 MB/s, yuku-parser 51.9 MB/s, yuku-parser-wasm 67.9 MB/s, swc 13.4 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 9.6x tsv-internal, tsv_wasm-json 6.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ----------- | ---------- | - | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier    | 0.08       | 7 | 13203.90 | 13231.73 | 13240.80 | —        | —        | 13075.58 | 13249.75 | baseline              |
| tsv         | 1.73       | 7 | 578.62   | 579.54   | 587.75   | —        | —        | 577.12   | 589.26   | 22.8x                 |
| tsv_wasm    | 1.29       | 5 | 776.37   | 776.89   | 780.26   | —        | —        | 769.22   | 785.28   | 17.0x                 |
| oxfmt       | 1.14       | 6 | 874.98   | 875.79   | 876.83   | —        | —        | 871.68   | 877.62   | 15.1x                 |
| dprint-wasm | 0.32       | 5 | 3150.61  | 3151.20  | 3154.07  | —        | —        | 3144.70  | 3155.99  | 4.19x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.4 MB/s, tsv 31.1 MB/s, tsv_wasm 23.2 MB/s, oxfmt 20.6 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 65.19      | 233  | 15.38    | 15.75    | 23.34    | 23.67    | 24.36    | 14.94    | 24.55    | baseline                     |
| tsv-json          | 72.35      | 361  | 13.77    | 14.47    | 14.96    | 15.28    | 16.27    | 12.64    | 18.87    | 1.11x                        |
| tsv_wasm-json     | 73.73      | 366  | 13.38    | 14.19    | 14.65    | 14.75    | 15.28    | 12.59    | 18.96    | 1.13x                        |
| tsv-internal      | 340.04     | 1422 | 2.94     | 2.97     | 3.04     | 3.10     | 3.29     | 2.90     | 3.78     | 5.22x                        |
| tsv_wasm-internal | 223.54     | 897  | 4.47     | 4.52     | 4.63     | 4.70     | 4.88     | 4.43     | 5.22     | 3.43x                        |
| postcss           | 84.48      | 356  | 11.72    | 12.36    | 14.04    | 18.90    | 20.07    | 10.69    | 21.35    | 1.30x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 21.8 MB/s, tsv-json 24.2 MB/s, tsv_wasm-json 24.7 MB/s, tsv-internal 113.8 MB/s, tsv_wasm-internal 74.8 MB/s, postcss 28.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.7x tsv-internal, tsv_wasm-json 3.0x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.53       | 12  | 392.92   | 402.95   | 427.89   | 453.18   | 477.38   | 382.69   | 483.43   | baseline              |
| tsv        | 146.83     | 673 | 6.79     | 6.88     | 6.99     | 7.04     | 7.22     | 6.70     | 7.60     | 58.0x                 |
| tsv_wasm   | 102.83     | 457 | 9.69     | 9.84     | 9.95     | 10.02    | 10.23    | 9.58     | 11.73    | 40.6x                 |
| oxfmt      | 58.59      | 288 | 17.06    | 17.37    | 17.78    | 18.09    | 18.99    | 15.63    | 21.41    | 23.2x                 |
| malva-wasm | 18.37      | 81  | 54.29    | 55.14    | 55.59    | 56.01    | 56.42    | 53.49    | 56.51    | 7.26x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.8 MB/s, tsv 49.2 MB/s, tsv_wasm 34.4 MB/s, oxfmt 19.6 MB/s, malva-wasm 6.1 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 839.9 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 928.7 KB | 361.5 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 932.3 KB | — | — |
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
| format svelte (773f) | **45.3x** prettier, **49.7x** oxfmt |
| format typescript (2504f) | **22.8x** prettier, **1.51x** oxfmt |
| format css (49f) | **58.0x** prettier, **2.51x** oxfmt |
| parse svelte (773f) | **4.33x** svelte/compiler, **2.14x** rsvelte-parse |
| parse typescript (2503f) | **4.24x** acorn-typescript, **0.74x** oxc-parser, **0.29x** yuku-parser, **1.13x** swc |
| parse css (49f) | **1.11x** svelte/compiler, **0.86x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **32.3x** prettier |
| format typescript (2504f) | **17.0x** prettier, **4.06x** dprint-wasm |
| format css (49f) | **40.6x** prettier, **5.60x** malva-wasm |
| parse svelte (773f) | **4.19x** svelte/compiler |
| parse typescript (2503f) | **4.33x** acorn-typescript, **1.04x** oxc-parser-wasm, **0.23x** yuku-parser-wasm |
| parse css (49f) | **1.13x** svelte/compiler, **0.87x** postcss |

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
