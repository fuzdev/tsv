# tsv benchmark results

**Runtime:** bun

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · bun 1.3.14

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-20T02:58:53.219Z — tsv 0.2.0 (8a87d997)

**Corpus:** 773 Svelte (2.0 MB), 2505 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3327 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.44       | 7   | 694.91   | 703.75   | 715.14   | —        | —        | 691.09   | 728.97   | baseline                     |
| tsv-json                    | 6.06       | 30  | 164.72   | 167.45   | 169.37   | 171.18   | 195.42   | 158.44   | 205.23   | 4.22x                        |
| tsv_wasm-json               | 5.81       | 28  | 172.45   | 174.06   | 176.78   | 178.50   | 205.75   | 166.55   | 216.02   | 4.05x                        |
| tsv-json-no-locations       | 8.50       | 38  | 117.30   | 119.01   | 121.71   | 142.77   | 150.85   | 114.58   | 151.74   | 5.92x                        |
| tsv_wasm-json-no-locations  | 7.87       | 35  | 126.69   | 129.74   | 135.64   | 154.57   | 158.46   | 123.57   | 159.43   | 5.48x                        |
| tsv-internal                | 52.79      | 261 | 18.85    | 19.29    | 19.44    | 19.61    | 20.13    | 18.47    | 21.61    | 36.8x                        |
| tsv_wasm-internal           | 38.32      | 185 | 25.94    | 26.46    | 26.72    | 26.94    | 27.19    | 25.67    | 29.46    | 26.7x                        |
| rsvelte-parse               | 3.05       | 15  | 327.15   | 330.24   | 335.03   | 343.73   | 356.39   | 321.19   | 359.55   | 2.13x                        |
| rsvelte-parse-skip-expr-loc | 4.93       | 21  | 203.67   | 207.14   | 225.35   | 225.78   | 232.87   | 198.45   | 235.10   | 3.43x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 2.8 MB/s, tsv-json 11.9 MB/s, tsv_wasm-json 11.4 MB/s, tsv-json-no-locations 16.7 MB/s, tsv_wasm-json-no-locations 15.4 MB/s, tsv-internal 103.5 MB/s, tsv_wasm-internal 75.1 MB/s, rsvelte-parse 6.0 MB/s, rsvelte-parse-skip-expr-loc 9.7 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 8.7x tsv-internal, tsv_wasm-json 6.6x tsv_wasm-internal

## format/svelte

| Task Name | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| --------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier  | 0.26       | 7  | 3.93    | 3.98    | 4.00    | —       | —       | 3.76    | 4.02    | baseline              |
| tsv       | 12.93      | 61 | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.08    | 0.09    | 50.4x                 |
| tsv_wasm  | 9.32       | 46 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 36.4x                 |
| oxfmt     | 0.19       | 7  | 5.26    | 5.31    | 5.38    | —       | —       | 5.00    | 5.44    | 0.74x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.5 MB/s, tsv 25.4 MB/s, tsv_wasm 18.3 MB/s, oxfmt 0.4 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ----------------------------- |
| acorn-typescript           | 0.17       | 7  | 5810.38  | 5907.86  | 5944.59  | —        | —        | 5727.36  | 5986.10  | baseline                      |
| tsv-json                   | 0.73       | 5  | 1374.72  | 1377.15  | 1381.70  | —        | —        | 1367.03  | 1384.73  | 4.25x                         |
| tsv_wasm-json              | 0.72       | 4  | 1394.89  | 1398.64  | 1401.37  | —        | —        | 1393.90  | 1403.19  | 4.18x                         |
| tsv-json-no-locations      | 1.24       | 6  | 802.89   | 805.78   | 809.51   | —        | —        | 800.65   | 813.93   | 7.26x                         |
| tsv_wasm-json-no-locations | 1.17       | 6  | 856.56   | 859.73   | 860.01   | —        | —        | 845.47   | 860.01   | 6.83x                         |
| tsv-internal               | 8.12       | 37 | 122.89   | 123.83   | 126.00   | 126.25   | 126.64   | 122.09   | 126.77   | 47.4x                         |
| tsv_wasm-internal          | 6.02       | 25 | 166.13   | 166.87   | 169.23   | 169.51   | 171.63   | 164.88   | 172.46   | 35.1x                         |
| oxc-parser                 | 0.99       | 5  | 1006.98  | 1007.92  | 1010.28  | —        | —        | 999.95   | 1011.85  | 5.80x                         |
| yuku-parser                | 2.88       | 10 | 343.72   | 382.76   | 391.64   | 398.25   | 407.41   | 335.32   | 409.70   | 16.8x                         |
| yuku-parser-wasm           | 3.67       | 12 | 273.88   | 311.52   | 318.07   | 321.33   | 321.90   | 266.78   | 322.04   | 21.4x                         |
| swc                        | 0.71       | 5  | 1410.79  | 1412.09  | 1413.45  | —        | —        | 1405.89  | 1414.35  | 4.14x                         |

**Files (intersection):** 2502

**Throughput:** acorn-typescript 3.1 MB/s, tsv-json 13.1 MB/s, tsv_wasm-json 12.9 MB/s, tsv-json-no-locations 22.4 MB/s, tsv_wasm-json-no-locations 21.0 MB/s, tsv-internal 145.9 MB/s, tsv_wasm-internal 108.1 MB/s, oxc-parser 17.9 MB/s, yuku-parser 51.8 MB/s, yuku-parser-wasm 66.0 MB/s, swc 12.7 MB/s

**Coverage:** acorn-typescript 2502/2505 (99%), tsv-json 2505/2505 (100%), tsv_wasm-json 2505/2505 (100%), tsv-json-no-locations 2505/2505 (100%), tsv_wasm-json-no-locations 2505/2505 (100%), tsv-internal 2505/2505 (100%), tsv_wasm-internal 2505/2505 (100%), oxc-parser 2503/2505 (99%), yuku-parser 2503/2505 (99%), yuku-parser-wasm 2503/2505 (99%), swc 2502/2505 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.2x tsv-internal, tsv_wasm-json 8.4x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.06       | 7 | 15.82   | 15.85   | 15.92   | —       | —       | 15.11   | 16.02   | baseline              |
| tsv         | 1.72       | 7 | 0.58    | 0.58    | 0.59    | —       | —       | 0.58    | 0.59    | 26.9x                 |
| tsv_wasm    | 1.30       | 6 | 0.77    | 0.77    | 0.78    | —       | —       | 0.77    | 0.78    | 20.4x                 |
| oxfmt       | 0.97       | 5 | 1.03    | 1.03    | 1.04    | —       | —       | 1.02    | 1.04    | 15.2x                 |
| dprint-wasm | 0.32       | 5 | 3.15    | 3.15    | 3.15    | —       | —       | 3.14    | 3.16    | 4.98x                 |

**Files (intersection):** 2503

**Throughput:** prettier 1.1 MB/s, tsv 30.9 MB/s, tsv_wasm 23.3 MB/s, oxfmt 17.4 MB/s, dprint-wasm 5.7 MB/s

**Coverage:** prettier 2505/2505 (100%), tsv 2505/2505 (100%), tsv_wasm 2505/2505 (100%), oxfmt 2503/2505 (99%), dprint-wasm 2505/2505 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 67.47      | 283  | 14.78    | 15.08    | 19.16    | 20.90    | 25.97    | 14.49    | 27.95    | baseline                     |
| tsv-json          | 69.85      | 298  | 14.30    | 14.77    | 16.48    | 18.85    | 19.61    | 13.69    | 23.20    | 1.04x                        |
| tsv_wasm-json     | 70.31      | 295  | 14.21    | 14.47    | 15.43    | 18.87    | 19.45    | 13.72    | 22.68    | 1.04x                        |
| tsv-internal      | 337.40     | 1475 | 2.96     | 2.99     | 3.04     | 3.07     | 3.21     | 2.92     | 3.59     | 5.00x                        |
| tsv_wasm-internal | 233.03     | 995  | 4.29     | 4.33     | 4.43     | 4.53     | 4.74     | 4.26     | 8.48     | 3.45x                        |
| postcss           | 87.23      | 358  | 11.36    | 12.01    | 13.64    | 19.45    | 23.43    | 10.91    | 25.03    | 1.29x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 22.6 MB/s, tsv-json 23.4 MB/s, tsv_wasm-json 23.5 MB/s, tsv-internal 112.9 MB/s, tsv_wasm-internal 78.0 MB/s, postcss 29.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.8x tsv-internal, tsv_wasm-json 3.3x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 2.12       | 10  | 471.09   | 481.49   | 490.00   | 500.48   | 508.86   | 461.90   | 510.96   | baseline              |
| tsv        | 147.95     | 708 | 6.74     | 6.84     | 6.94     | 6.99     | 7.61     | 6.46     | 10.39    | 69.9x                 |
| tsv_wasm   | 102.76     | 463 | 9.69     | 9.87     | 10.04    | 10.12    | 10.41    | 9.59     | 10.83    | 48.6x                 |
| oxfmt      | 48.91      | 245 | 20.10    | 22.17    | 23.31    | 23.74    | 24.98    | 17.31    | 26.71    | 23.1x                 |
| malva-wasm | 17.80      | 88  | 55.92    | 56.73    | 57.41    | 57.87    | 59.28    | 55.24    | 63.13    | 8.41x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.7 MB/s, tsv 49.5 MB/s, tsv_wasm 34.4 MB/s, oxfmt 16.4 MB/s, malva-wasm 6.0 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 825.5 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 927.5 KB | 364.0 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 917.2 KB | — | — |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.7x | 1.3x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.6x | 0.5x |
| yuku-parser (wasm) | 673.9 KB | 200.3 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.6x | 0.5x |
| tsv (ffi) | 3.5 MB | 1.5 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 666.1 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.8x |
| oxc-parser (napi) | 2.1 MB | 885.7 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.0 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 741.1 KB | 310.4 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.3 MB | 3.3 MB | 2.2x | 2.0x |
| rsvelte compiler (napi) | 14.5 MB | 6.0 MB | 3.8x | 3.7x |
| swc (napi) | 31.9 MB | 11.9 MB | 8.4x | 7.3x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **50.4x** prettier, **67.7x** oxfmt |
| format typescript (2503f) | **26.9x** prettier, **1.77x** oxfmt |
| format css (49f) | **69.9x** prettier, **3.03x** oxfmt |
| parse svelte (773f) | **4.22x** svelte/compiler, **1.99x** rsvelte-parse |
| parse typescript (2502f) | **4.25x** acorn-typescript, **0.73x** oxc-parser, **0.25x** yuku-parser, **1.03x** swc |
| parse css (49f) | **1.04x** svelte/compiler, **0.80x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **36.4x** prettier |
| format typescript (2503f) | **20.4x** prettier, **4.09x** dprint-wasm |
| format css (49f) | **48.6x** prettier, **5.77x** malva-wasm |
| parse svelte (773f) | **4.05x** svelte/compiler |
| parse typescript (2502f) | **4.18x** acorn-typescript, **0.19x** yuku-parser-wasm |
| parse css (49f) | **1.04x** svelte/compiler, **0.81x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload on a real component, so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

12 unique file+error combinations — Svelte 0, TypeScript 12, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
