# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-24T15:38:47.288Z — tsv 0.2.0 (831e5193)

**Corpus:** 773 Svelte (2.0 MB), 2506 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3328 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (416), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.04       | 10  | 486.23   | 499.71   | 505.61   | 508.12   | 510.13   | 482.05   | 510.64   | baseline                     |
| tsv-json                    | 4.64       | 22  | 214.30   | 218.88   | 220.38   | 221.88   | 223.04   | 212.85   | 223.33   | 2.27x                        |
| tsv_wasm-json               | 4.39       | 18  | 227.99   | 229.92   | 233.48   | 234.05   | 234.35   | 226.44   | 234.43   | 2.15x                        |
| tsv-json-no-locations       | 7.49       | 33  | 133.42   | 134.70   | 137.02   | 137.23   | 137.34   | 131.80   | 137.36   | 3.67x                        |
| tsv_wasm-json-no-locations  | 6.69       | 34  | 148.78   | 151.00   | 152.20   | 152.81   | 153.32   | 146.86   | 153.36   | 3.28x                        |
| tsv-internal                | 49.04      | 173 | 20.35    | 21.02    | 21.19    | 21.28    | 21.37    | 20.22    | 21.63    | 24.0x                        |
| tsv_wasm-internal           | 34.82      | 160 | 28.55    | 29.23    | 29.43    | 29.50    | 29.64    | 28.34    | 29.77    | 17.1x                        |
| rsvelte-parse               | 2.67       | 12  | 373.43   | 375.81   | 384.25   | 385.37   | 385.67   | 371.49   | 385.75   | 1.31x                        |
| rsvelte-parse-skip-expr-loc | 4.52       | 23  | 220.28   | 221.88   | 226.00   | 226.22   | 226.91   | 217.88   | 227.10   | 2.22x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 9.1 MB/s, tsv_wasm-json 8.6 MB/s, tsv-json-no-locations 14.7 MB/s, tsv_wasm-json-no-locations 13.1 MB/s, tsv-internal 96.2 MB/s, tsv_wasm-internal 68.3 MB/s, rsvelte-parse 5.2 MB/s, rsvelte-parse-skip-expr-loc 8.9 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.6x tsv-internal, tsv_wasm-json 7.9x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22       | 6  | 4508.27  | 4571.65  | 4647.40  | —        | —        | 4463.30  | 4702.40  | baseline              |
| tsv        | 13.16      | 54 | 75.86    | 77.44    | 78.33    | 78.46    | 79.13    | 75.36    | 80.21    | 59.4x                 |
| tsv_wasm   | 9.36       | 39 | 106.66   | 108.80   | 110.40   | 111.02   | 111.42   | 105.83   | 111.44   | 42.2x                 |
| oxfmt      | 0.22       | 6  | 4549.77  | 4588.11  | 4653.87  | —        | —        | 4504.50  | 4717.48  | 0.99x                 |
| biome-wasm | 1.06       | 6  | 939.56   | 942.72   | 946.42   | —        | —        | 926.34   | 949.26   | 4.81x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.4 MB/s, tsv 25.8 MB/s, tsv_wasm 18.4 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.30       | 5  | 3.38    | 3.39    | 3.39    | —       | —       | 3.37    | 3.40    | baseline                      |
| tsv-json                   | 0.48       | 4  | 2.09    | 2.09    | 2.09    | —       | —       | 2.09    | 2.09    | 1.62x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.15    | 2.16    | 2.16    | —       | —       | 2.14    | 2.16    | 1.57x                         |
| tsv-json-no-locations      | 0.98       | 4  | 1.02    | 1.02    | 1.03    | —       | —       | 1.02    | 1.03    | 3.31x                         |
| tsv_wasm-json-no-locations | 0.91       | 5  | 1.10    | 1.10    | 1.10    | —       | —       | 1.10    | 1.10    | 3.08x                         |
| tsv-internal               | 7.04       | 28 | 0.14    | 0.14    | 0.15    | 0.15    | 0.15    | 0.14    | 0.15    | 23.8x                         |
| tsv_wasm-internal          | 5.46       | 23 | 0.18    | 0.18    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 18.5x                         |
| oxc-parser                 | 0.74       | 5  | 1.36    | 1.36    | 1.36    | —       | —       | 1.34    | 1.37    | 2.50x                         |
| oxc-parser-wasm            | 0.72       | 4  | 1.40    | 1.40    | 1.41    | —       | —       | 1.39    | 1.41    | 2.42x                         |
| yuku-parser                | 2.44       | 10 | 0.41    | 0.42    | 0.42    | 0.43    | 0.43    | 0.41    | 0.43    | 8.27x                         |
| yuku-parser-wasm           | 2.89       | 11 | 0.35    | 0.35    | 0.36    | 0.37    | 0.37    | 0.34    | 0.37    | 9.79x                         |
| swc                        | 0.57       | 5  | 1.75    | 1.75    | 1.75    | —       | —       | 1.74    | 1.75    | 1.94x                         |

**Files (intersection):** 2503

**Throughput:** acorn-typescript 5.3 MB/s, tsv-json 8.6 MB/s, tsv_wasm-json 8.4 MB/s, tsv-json-no-locations 17.6 MB/s, tsv_wasm-json-no-locations 16.4 MB/s, tsv-internal 126.6 MB/s, tsv_wasm-internal 98.2 MB/s, oxc-parser 13.3 MB/s, oxc-parser-wasm 12.9 MB/s, yuku-parser 44.0 MB/s, yuku-parser-wasm 52.0 MB/s, swc 10.3 MB/s

**Coverage:** acorn-typescript 2503/2506 (99%), tsv-json 2506/2506 (100%), tsv_wasm-json 2506/2506 (100%), tsv-json-no-locations 2506/2506 (100%), tsv_wasm-json-no-locations 2506/2506 (100%), tsv-internal 2506/2506 (100%), tsv_wasm-internal 2506/2506 (100%), oxc-parser 2504/2506 (99%), oxc-parser-wasm 2504/2506 (99%), yuku-parser 2504/2506 (99%), yuku-parser-wasm 2504/2506 (99%), swc 2503/2506 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.7x tsv-internal, tsv_wasm-json 11.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 7 | 14.12   | 14.21   | 14.29   | —       | —       | 14.04   | 14.38   | baseline              |
| tsv         | 1.73       | 7 | 0.58    | 0.58    | 0.59    | —       | —       | 0.58    | 0.59    | 24.4x                 |
| tsv_wasm    | 1.28       | 6 | 0.78    | 0.78    | 0.79    | —       | —       | 0.78    | 0.79    | 18.2x                 |
| oxfmt       | 1.14       | 6 | 0.87    | 0.88    | 0.88    | —       | —       | 0.87    | 0.88    | 16.2x                 |
| biome-wasm  | 0.22       | 3 | 4.61    | 10.16   | 11.79   | —       | —       | 4.56    | 12.87   | 3.08x                 |
| dprint-wasm | 0.31       | 5 | 3.22    | 3.23    | 3.23    | —       | —       | 3.21    | 3.23    | 4.40x                 |

**Files (intersection):** 2504

**Throughput:** prettier 1.3 MB/s, tsv 31.1 MB/s, tsv_wasm 23.1 MB/s, oxfmt 20.6 MB/s, biome-wasm 3.9 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2506/2506 (100%), tsv 2506/2506 (100%), tsv_wasm 2506/2506 (100%), oxfmt 2504/2506 (99%), biome-wasm 2506/2506 (100%), dprint-wasm 2506/2506 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 106.41     | 494  | 9.29     | 9.69     | 10.15    | 10.45    | 15.38    | 8.99     | 19.38    | baseline                     |
| tsv-json          | 55.59      | 252  | 17.94    | 18.30    | 18.57    | 20.43    | 21.24    | 17.17    | 22.08    | 0.52x                        |
| tsv_wasm-json     | 53.24      | 236  | 18.75    | 19.08    | 20.10    | 20.75    | 25.61    | 18.31    | 26.22    | 0.50x                        |
| tsv-internal      | 290.88     | 1209 | 3.43     | 3.47     | 3.53     | 3.57     | 3.62     | 3.40     | 3.94     | 2.73x                        |
| tsv_wasm-internal | 204.82     | 880  | 4.88     | 4.92     | 4.99     | 5.02     | 5.10     | 4.81     | 5.59     | 1.92x                        |
| postcss           | 99.56      | 482  | 9.98     | 10.28    | 10.55    | 10.78    | 11.42    | 9.73     | 13.09    | 0.94x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 35.6 MB/s, tsv-json 18.6 MB/s, tsv_wasm-json 17.8 MB/s, tsv-internal 97.4 MB/s, tsv_wasm-internal 68.6 MB/s, postcss 33.3 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.2x tsv-internal, tsv_wasm-json 3.8x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.78       | 9   | 559.98   | 564.13   | 578.12   | —        | —        | 541.50   | 581.96   | baseline              |
| tsv        | 140.58     | 594 | 7.10     | 7.20     | 7.35     | 7.41     | 7.65     | 7.04     | 10.37    | 78.8x                 |
| tsv_wasm   | 99.25      | 474 | 10.03    | 10.18    | 10.28    | 10.34    | 10.62    | 9.93     | 13.13    | 55.6x                 |
| oxfmt      | 58.18      | 287 | 17.17    | 17.53    | 17.86    | 18.13    | 19.03    | 15.83    | 20.33    | 32.6x                 |
| biome-wasm | 6.07       | 23  | 139.79   | 222.92   | 238.75   | 240.96   | 242.14   | 135.83   | 242.25   | 3.40x                 |
| malva-wasm | 21.84      | 104 | 45.55    | 46.41    | 47.03    | 47.60    | 48.22    | 45.07    | 48.61    | 12.2x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 47.1 MB/s, tsv_wasm 33.2 MB/s, oxfmt 19.5 MB/s, biome-wasm 2.0 MB/s, malva-wasm 7.3 MB/s

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
| tsv (ffi) | 3.5 MB | 1.5 MB | 0.9x | 0.9x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.8x | 0.8x |
| tsv parse (ffi) | 1.5 MB | 657.5 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.7 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 2.9x | 2.8x |
| oxc-parser (napi) | 2.1 MB | 885.7 KB | 0.6x | 0.5x |
| oxfmt (napi) | 9.0 MB | 3.7 MB | 2.4x | 2.2x |
| yuku-parser (napi) | 741.1 KB | 310.4 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.3 MB | 3.3 MB | 2.2x | 2.0x |
| rsvelte compiler (napi) | 14.5 MB | 6.0 MB | 3.8x | 3.6x |
| swc (napi) | 31.9 MB | 11.9 MB | 8.3x | 7.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **59.4x** prettier, **59.9x** oxfmt |
| format typescript (2504f) | **24.4x** prettier, **1.51x** oxfmt |
| format css (49f) | **78.8x** prettier, **2.42x** oxfmt |
| parse svelte (773f) | **2.27x** svelte/compiler, **1.73x** rsvelte-parse |
| parse typescript (2503f) | **1.62x** acorn-typescript, **0.65x** oxc-parser, **0.20x** yuku-parser, **0.84x** swc |
| parse css (49f) | **0.52x** svelte/compiler, **0.56x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **42.2x** prettier, **8.79x** biome-wasm |
| format typescript (2504f) | **18.2x** prettier, **5.88x** biome-wasm, **4.13x** dprint-wasm |
| format css (49f) | **55.6x** prettier, **16.3x** biome-wasm, **4.54x** malva-wasm |
| parse svelte (773f) | **2.15x** svelte/compiler |
| parse typescript (2503f) | **1.57x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.16x** yuku-parser-wasm |
| parse css (49f) | **0.50x** svelte/compiler, **0.53x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload measured across the corpus (the axis a throughput ratio integrates; per component the spread is wider), so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Unstable Rows

1 timed row(s) varied more than 10% across iterations (cv = std_dev / mean, post-outlier-removal). Every `Nx` involving one of these divides an unstable mean — read it as approximate, and prefer re-running before drawing a conclusion from it.

| Row | cv | samples |
| --- | ---: | ---: |
| format/css/biome-wasm | 23.5% | 23 |

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
