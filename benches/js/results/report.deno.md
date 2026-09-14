# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.6

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-09-14T22:11:04.153Z — tsv 0.3.0 (7eb42464)

**Corpus:** 951 Svelte (2.4 MB), 2645 TypeScript (18.5 MB), 55 CSS (0.4 MB) — 3651 files, 21.3 MB total

**Corpus snapshot:** [fuzdev/corpora@e8d37ffc1](https://github.com/fuzdev/corpora/tree/e8d37ffc18b8b8b743aaa93eca015871a7f6e98a) — the real-code sources are its collections, vendored at this commit

**Sources:** ../corpora/collections/zzz/src (326), ../corpora/collections/fuz_app/src (695), ../corpora/collections/fuz_blog/src (38), ../corpora/collections/fuz_code/src (66), ../corpora/collections/fuz_css/src (170), ../corpora/collections/fuz_docs/src (66), ../corpora/collections/fuz_gitops/src (99), ../corpora/collections/fuz_mastodon/src (25), ../corpora/collections/fuz_template/src (18), ../corpora/collections/fuz_ui/src (236), ../corpora/collections/fuz_util/src (147), ../corpora/collections/mdz/src (70), ../corpora/collections/gro/src (161), ../corpora/collections/svelte-docinfo/src (119), ../corpora/collections/tsv.fuz.dev/src (33), ../corpora/collections/ryanatkn.com/src (52), ../corpora/collections/webdevladder.net/src (39), ../corpora/collections/earbetter/src (72), ../corpora/collections/cosmicplayground/src (217), benches/js/.cache/svelte_styles (20), ../corpora/collections/kit/packages/kit/src (298), ../corpora/collections/svelte/packages/svelte/src (416), ../corpora/collections/svelte.dev/apps/svelte.dev/src (145), ../corpora/collections/svelte.dev/packages/repl/src (53), ../corpora/collections/svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.150.0, @oxc-parser/binding-wasm32-wasi@0.142.0, oxfmt@0.68.0, yuku-parser@0.10.1, @biomejs/wasm-bundler@2.5.13, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.28, @rsvelte/fmt@0.7.23, @rsvelte/vite-plugin-svelte-native@0.3.14 (targets svelte@5.57.0), @swc/core@1.16.2

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 1.62       | 9   | 613.09   | 624.15   | 642.32   | —        | —        | 600.47   | 645.18   | baseline                     |
| tsv-json                    | 4.16       | 17  | 240.66   | 241.23   | 246.38   | 246.50   | 246.63   | 238.77   | 246.66   | 2.57x                        |
| tsv_wasm-json               | 3.54       | 14  | 282.92   | 284.42   | 289.38   | 289.51   | 289.58   | 281.46   | 289.59   | 2.19x                        |
| tsv-json-no-locations       | 6.69       | 34  | 148.83   | 150.72   | 153.20   | 153.50   | 153.80   | 147.20   | 153.92   | 4.14x                        |
| tsv_wasm-json-no-locations  | 5.39       | 23  | 185.61   | 187.00   | 189.81   | 190.44   | 191.44   | 184.25   | 191.72   | 3.33x                        |
| tsv-internal                | 47.43      | 188 | 21.02    | 21.49    | 21.81    | 22.50    | 22.87    | 20.90    | 23.08    | 29.3x                        |
| tsv_wasm-internal           | 28.41      | 116 | 35.11    | 35.81    | 36.13    | 36.51    | 37.09    | 34.83    | 37.85    | 17.6x                        |
| rsvelte-parse               | 1.80       | 7   | 555.35   | 558.62   | 565.47   | —        | —        | 553.23   | 571.12   | 1.11x                        |
| rsvelte-parse-skip-expr-loc | 2.74       | 13  | 364.32   | 366.09   | 367.52   | 370.95   | 375.69   | 361.14   | 376.87   | 1.70x                        |

**Files (intersection):** 951

**Throughput:** svelte/compiler 3.9 MB/s, tsv-json 10.0 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 16.1 MB/s, tsv_wasm-json-no-locations 13.0 MB/s, tsv-internal 114.4 MB/s, tsv_wasm-internal 68.5 MB/s, rsvelte-parse 4.3 MB/s, rsvelte-parse-skip-expr-loc 6.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 11.4x tsv-internal, tsv_wasm-json 8.0x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.19       | 6  | 5329.59  | 5455.58  | 5542.46  | —        | —        | 5294.08  | 5653.23  | baseline              |
| tsv        | 12.61      | 56 | 78.88    | 81.19    | 82.60    | 83.29    | 83.74    | 78.03    | 84.40    | 67.6x                 |
| tsv_wasm   | 7.83       | 38 | 127.23   | 128.98   | 130.85   | 131.34   | 132.77   | 126.15   | 133.00   | 41.9x                 |
| oxfmt      | 0.19       | 6  | 5370.44  | 5445.04  | 5549.57  | —        | —        | 5326.44  | 5643.02  | 1.00x                 |
| biome-wasm | 1.10       | 5  | 912.41   | 912.90   | 915.10   | —        | —        | 909.75   | 917.15   | 5.88x                 |

**Files (intersection):** 951

**Throughput:** prettier 0.5 MB/s, tsv 30.4 MB/s, tsv_wasm 18.9 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 951/951 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.30       | 4  | 3.30    | 3.30    | 3.30    | —       | —       | 3.27    | 3.30    | baseline                      |
| tsv-json                   | 0.53       | 4  | 1.88    | 1.88    | 1.88    | —       | —       | 1.87    | 1.89    | 1.76x                         |
| tsv_wasm-json              | 0.47       | 4  | 2.11    | 2.11    | 2.12    | —       | —       | 2.10    | 2.13    | 1.56x                         |
| tsv-json-no-locations      | 1.11       | 6  | 0.90    | 0.91    | 0.91    | —       | —       | 0.90    | 0.91    | 3.64x                         |
| tsv_wasm-json-no-locations | 0.93       | 5  | 1.08    | 1.08    | 1.08    | —       | —       | 1.08    | 1.08    | 3.05x                         |
| tsv-internal               | 9.10       | 35 | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 0.11    | 30.0x                         |
| tsv_wasm-internal          | 5.68       | 27 | 0.18    | 0.18    | 0.18    | 0.18    | 0.18    | 0.17    | 0.18    | 18.7x                         |
| oxc-parser                 | 0.80       | 5  | 1.25    | 1.26    | 1.27    | —       | —       | 1.22    | 1.28    | 2.64x                         |
| oxc-parser-wasm            | 0.71       | 4  | 1.40    | 1.40    | 1.40    | —       | —       | 1.40    | 1.41    | 2.35x                         |
| yuku-parser                | 2.13       | 10 | 0.47    | 0.47    | 0.49    | 0.49    | 0.49    | 0.45    | 0.49    | 7.01x                         |
| yuku-parser-wasm           | 2.34       | 12 | 0.43    | 0.43    | 0.44    | 0.45    | 0.47    | 0.41    | 0.47    | 7.71x                         |
| swc                        | 0.58       | 4  | 1.74    | 1.74    | 1.76    | —       | —       | 1.74    | 1.76    | 1.89x                         |

**Files (intersection):** 2642

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 9.8 MB/s, tsv_wasm-json 8.8 MB/s, tsv-json-no-locations 20.4 MB/s, tsv_wasm-json-no-locations 17.1 MB/s, tsv-internal 167.8 MB/s, tsv_wasm-internal 104.8 MB/s, oxc-parser 14.8 MB/s, oxc-parser-wasm 13.2 MB/s, yuku-parser 39.3 MB/s, yuku-parser-wasm 43.2 MB/s, swc 10.6 MB/s

**Coverage:** acorn-typescript 2642/2645 (99%), tsv-json 2645/2645 (100%), tsv_wasm-json 2645/2645 (100%), tsv-json-no-locations 2645/2645 (100%), tsv_wasm-json-no-locations 2645/2645 (100%), tsv-internal 2645/2645 (100%), tsv_wasm-internal 2645/2645 (100%), oxc-parser 2643/2645 (99%), oxc-parser-wasm 2643/2645 (99%), yuku-parser 2643/2645 (99%), yuku-parser-wasm 2643/2645 (99%), swc 2642/2645 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 17.1x tsv-internal, tsv_wasm-json 12.0x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 7  | 13.22   | 13.27   | 13.33   | —       | —       | 12.99   | 13.38   | baseline              |
| tsv         | 2.30       | 10 | 0.43    | 0.44    | 0.45    | 0.45    | 0.45    | 0.43    | 0.45    | 30.3x                 |
| tsv_wasm    | 1.39       | 6  | 0.72    | 0.72    | 0.73    | —       | —       | 0.72    | 0.74    | 18.4x                 |
| oxfmt       | 1.09       | 6  | 0.91    | 0.92    | 0.93    | —       | —       | 0.90    | 0.93    | 14.4x                 |
| biome-wasm  | 0.22       | 5  | 4.54    | 4.54    | 4.54    | —       | —       | 4.53    | 4.54    | 2.91x                 |
| dprint-wasm | 0.27       | 5  | 3.67    | 3.67    | 3.67    | —       | —       | 3.66    | 3.67    | 3.60x                 |

**Files (intersection):** 2643

**Throughput:** prettier 1.4 MB/s, tsv 42.4 MB/s, tsv_wasm 25.7 MB/s, oxfmt 20.2 MB/s, biome-wasm 4.1 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2645/2645 (100%), tsv 2645/2645 (100%), tsv_wasm 2645/2645 (100%), oxfmt 2643/2645 (99%), biome-wasm 2645/2645 (100%), dprint-wasm 2645/2645 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 90.01      | 443  | 11.05    | 11.34    | 11.63    | 11.82    | 12.43    | 10.61    | 14.78    | baseline                     |
| tsv-json          | 48.71      | 233  | 20.48    | 20.76    | 21.23    | 21.69    | 22.40    | 20.07    | 23.26    | 0.54x                        |
| tsv_wasm-json     | 38.96      | 183  | 25.61    | 26.05    | 26.57    | 27.66    | 27.96    | 24.96    | 29.38    | 0.43x                        |
| tsv-internal      | 228.49     | 1131 | 4.37     | 4.40     | 4.45     | 4.48     | 4.52     | 4.31     | 8.38     | 2.54x                        |
| tsv_wasm-internal | 127.34     | 543  | 7.84     | 7.92     | 7.99     | 8.01     | 8.06     | 7.80     | 8.15     | 1.41x                        |
| postcss           | 81.03      | 399  | 12.32    | 12.56    | 12.80    | 13.03    | 13.43    | 11.85    | 16.06    | 0.90x                        |

**Files (intersection):** 55

**Throughput:** svelte/compiler 36.5 MB/s, tsv-json 19.7 MB/s, tsv_wasm-json 15.8 MB/s, tsv-internal 92.6 MB/s, tsv_wasm-internal 51.6 MB/s, postcss 32.8 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 4.7x tsv-internal, tsv_wasm-json 3.3x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.61       | 9   | 616.93   | 630.70   | 638.39   | —        | —        | 606.21   | 658.57   | baseline              |
| tsv        | 130.76     | 625 | 7.61     | 7.75     | 7.87     | 7.93     | 8.03     | 7.50     | 8.26     | 81.3x                 |
| tsv_wasm   | 72.93      | 341 | 13.65    | 13.89    | 13.95    | 13.99    | 14.13    | 13.53    | 14.48    | 45.3x                 |
| oxfmt      | 48.29      | 242 | 20.67    | 21.12    | 21.58    | 21.98    | 22.31    | 18.81    | 22.54    | 30.0x                 |
| biome-wasm | 9.81       | 40  | 101.77   | 103.33   | 104.79   | 105.03   | 107.23   | 100.57   | 108.95   | 6.10x                 |
| malva-wasm | 17.44      | 64  | 57.28    | 58.18    | 58.71    | 58.77    | 59.26    | 57.05    | 59.27    | 10.8x                 |

**Files (intersection):** 55

**Throughput:** prettier 0.7 MB/s, tsv 53.0 MB/s, tsv_wasm 29.5 MB/s, oxfmt 19.6 MB/s, biome-wasm 4.0 MB/s, malva-wasm 7.1 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.5 MB | 928.4 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 995.6 KB | 384.6 KB | 0.4x | 0.4x |
| tsv_wasm | 2.8 MB | 1.0 MB | — | — |
| biome (wasm) | 44.6 MB | 11.4 MB | 15.9x | 11.1x |
| dprint (wasm) | 4.2 MB | 1.2 MB | 1.5x | 1.1x |
| oxc-parser (wasm) | 1.5 MB | 481.4 KB | 0.5x | 0.5x |
| yuku-parser (wasm) | 743.3 KB | 222.8 KB | 0.3x | 0.2x |
| malva (wasm) | 1.5 MB | 414.0 KB | 0.5x | 0.4x |
| tsv (ffi) | 3.5 MB | 1.6 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 2.8x |
| tsv format (ffi) | 3.2 MB | 1.5 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.6 MB | 684.0 KB | 0.4x | 0.4x |
| tsv (napi) | 3.9 MB | 1.8 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 882.6 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.1 MB | 3.7 MB | 2.6x | 2.3x |
| yuku-parser (napi) | 819.2 KB | 338.1 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.9 MB | 3.5 MB | 2.5x | 2.2x |
| rsvelte compiler (napi) | 17.6 MB | 7.4 MB | 5.0x | 4.6x |
| swc (napi) | 32.7 MB | 12.2 MB | 9.3x | 7.6x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **67.6x** prettier, **67.9x** oxfmt |
| format typescript (2643f) | **30.3x** prettier, **2.10x** oxfmt |
| format css (55f) | **81.3x** prettier, **2.71x** oxfmt |
| parse svelte (951f) | **2.57x** svelte/compiler, **2.31x** rsvelte-parse |
| parse typescript (2642f) | **1.76x** acorn-typescript, **0.66x** oxc-parser, **0.25x** yuku-parser, **0.93x** swc |
| parse css (55f) | **0.54x** svelte/compiler, **0.60x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (951f) | **41.9x** prettier, **7.14x** biome-wasm |
| format typescript (2643f) | **18.4x** prettier, **6.30x** biome-wasm, **5.10x** dprint-wasm |
| format css (55f) | **45.3x** prettier, **7.43x** biome-wasm, **4.18x** malva-wasm |
| parse svelte (951f) | **2.19x** svelte/compiler |
| parse typescript (2642f) | **1.56x** acorn-typescript, **0.67x** oxc-parser-wasm, **0.20x** yuku-parser-wasm |
| parse css (55f) | **0.43x** svelte/compiler, **0.48x** postcss |

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
