# tsv benchmark results

**Runtime:** node

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · node 24.14.1

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-20T02:49:33.411Z — tsv 0.2.0 (8a87d997)

**Corpus:** 773 Svelte (2.0 MB), 2505 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3327 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.05       | 11  | 484.75   | 498.08   | 501.71   | 508.34   | 513.64   | 474.13   | 514.97   | baseline                     |
| tsv-json                    | 4.56       | 20  | 219.06   | 219.70   | 222.78   | 223.08   | 223.53   | 218.15   | 223.65   | 2.23x                        |
| tsv_wasm-json               | 4.29       | 19  | 233.12   | 234.27   | 237.26   | 237.91   | 238.23   | 231.56   | 238.30   | 2.10x                        |
| tsv-json-no-locations       | 7.23       | 34  | 138.11   | 138.86   | 141.08   | 141.30   | 141.48   | 136.48   | 141.51   | 3.53x                        |
| tsv_wasm-json-no-locations  | 6.51       | 33  | 153.26   | 153.79   | 156.34   | 156.51   | 156.99   | 151.28   | 157.20   | 3.19x                        |
| tsv-internal                | 49.41      | 200 | 20.15    | 20.70    | 20.89    | 20.97    | 21.11    | 20.02    | 21.46    | 24.2x                        |
| tsv_wasm-internal           | 35.96      | 170 | 27.67    | 28.17    | 28.36    | 28.44    | 28.57    | 27.44    | 28.75    | 17.6x                        |
| rsvelte-parse               | 2.69       | 12  | 371.15   | 373.07   | 378.98   | 380.78   | 381.43   | 368.70   | 381.59   | 1.32x                        |
| rsvelte-parse-skip-expr-loc | 4.57       | 22  | 217.85   | 219.55   | 222.27   | 223.05   | 224.09   | 216.35   | 224.36   | 2.24x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 8.9 MB/s, tsv_wasm-json 8.4 MB/s, tsv-json-no-locations 14.2 MB/s, tsv_wasm-json-no-locations 12.8 MB/s, tsv-internal 96.9 MB/s, tsv_wasm-internal 70.5 MB/s, rsvelte-parse 5.3 MB/s, rsvelte-parse-skip-expr-loc 9.0 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.8x tsv-internal, tsv_wasm-json 8.4x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.22       | 7  | 4457.53  | 4482.96  | 4557.11  | —        | —        | 4415.47  | 4640.82  | baseline              |
| tsv        | 13.27      | 63 | 75.04    | 76.00    | 76.88    | 76.93    | 76.97    | 74.60    | 76.99    | 59.4x                 |
| tsv_wasm   | 9.33       | 41 | 106.79   | 108.57   | 110.27   | 110.94   | 111.31   | 106.03   | 111.39   | 41.8x                 |
| oxfmt      | 0.22       | 6  | 4489.35  | 4547.56  | 4614.37  | —        | —        | 4459.05  | 4699.18  | 0.99x                 |
| biome-wasm | 1.07       | 6  | 930.21   | 940.52   | 945.91   | —        | —        | 915.34   | 947.95   | 4.80x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.4 MB/s, tsv 26.0 MB/s, tsv_wasm 18.3 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.1 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.30       | 5  | 3.33    | 3.34    | 3.34    | —       | —       | 3.33    | 3.34    | baseline                      |
| tsv-json                   | 0.48       | 3  | 2.09    | 2.09    | 2.10    | —       | —       | 2.09    | 2.10    | 1.59x                         |
| tsv_wasm-json              | 0.46       | 4  | 2.15    | 2.16    | 2.16    | —       | —       | 2.15    | 2.16    | 1.55x                         |
| tsv-json-no-locations      | 0.97       | 4  | 1.03    | 1.03    | 1.03    | —       | —       | 1.03    | 1.03    | 3.24x                         |
| tsv_wasm-json-no-locations | 0.91       | 5  | 1.10    | 1.10    | 1.10    | —       | —       | 1.10    | 1.10    | 3.03x                         |
| tsv-internal               | 7.05       | 29 | 0.14    | 0.14    | 0.15    | 0.15    | 0.15    | 0.14    | 0.15    | 23.5x                         |
| tsv_wasm-internal          | 5.50       | 23 | 0.18    | 0.18    | 0.19    | 0.19    | 0.19    | 0.18    | 0.19    | 18.3x                         |
| oxc-parser                 | 0.74       | 5  | 1.35    | 1.36    | 1.36    | —       | —       | 1.33    | 1.36    | 2.47x                         |
| oxc-parser-wasm            | 0.71       | 4  | 1.40    | 1.40    | 1.41    | —       | —       | 1.40    | 1.41    | 2.38x                         |
| yuku-parser                | 2.44       | 12 | 0.41    | 0.41    | 0.42    | 0.43    | 0.43    | 0.40    | 0.43    | 8.12x                         |
| yuku-parser-wasm           | 2.86       | 15 | 0.35    | 0.35    | 0.36    | 0.36    | 0.36    | 0.34    | 0.36    | 9.55x                         |
| swc                        | 0.58       | 5  | 1.74    | 1.74    | 1.74    | —       | —       | 1.74    | 1.74    | 1.92x                         |

**Files (intersection):** 2502

**Throughput:** acorn-typescript 5.4 MB/s, tsv-json 8.6 MB/s, tsv_wasm-json 8.3 MB/s, tsv-json-no-locations 17.5 MB/s, tsv_wasm-json-no-locations 16.3 MB/s, tsv-internal 126.5 MB/s, tsv_wasm-internal 98.7 MB/s, oxc-parser 13.3 MB/s, oxc-parser-wasm 12.8 MB/s, yuku-parser 43.8 MB/s, yuku-parser-wasm 51.4 MB/s, swc 10.3 MB/s

**Coverage:** acorn-typescript 2502/2505 (99%), tsv-json 2505/2505 (100%), tsv_wasm-json 2505/2505 (100%), tsv-json-no-locations 2505/2505 (100%), tsv_wasm-json-no-locations 2505/2505 (100%), tsv-internal 2505/2505 (100%), tsv_wasm-internal 2505/2505 (100%), oxc-parser 2503/2505 (99%), oxc-parser-wasm 2503/2505 (99%), yuku-parser 2503/2505 (99%), yuku-parser-wasm 2503/2505 (99%), swc 2502/2505 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.8x tsv-internal, tsv_wasm-json 11.8x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.07       | 7 | 13.98   | 14.03   | 14.07   | —       | —       | 13.95   | 14.13   | baseline              |
| tsv         | 1.74       | 7 | 0.58    | 0.58    | 0.59    | —       | —       | 0.57    | 0.59    | 24.3x                 |
| tsv_wasm    | 1.26       | 6 | 0.79    | 0.79    | 0.80    | —       | —       | 0.79    | 0.81    | 17.7x                 |
| oxfmt       | 1.14       | 6 | 0.88    | 0.88    | 0.89    | —       | —       | 0.87    | 0.89    | 16.0x                 |
| biome-wasm  | 0.22       | 3 | 4.60    | 9.95    | 11.47   | —       | —       | 4.51    | 12.48   | 3.08x                 |
| dprint-wasm | 0.31       | 5 | 3.20    | 3.21    | 3.21    | —       | —       | 3.20    | 3.21    | 4.37x                 |

**Files (intersection):** 2503

**Throughput:** prettier 1.3 MB/s, tsv 31.2 MB/s, tsv_wasm 22.7 MB/s, oxfmt 20.5 MB/s, biome-wasm 4.0 MB/s, dprint-wasm 5.6 MB/s

**Coverage:** prettier 2505/2505 (100%), tsv 2505/2505 (100%), tsv_wasm 2505/2505 (100%), oxfmt 2503/2505 (99%), biome-wasm 2505/2505 (100%), dprint-wasm 2505/2505 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 106.53     | 500  | 9.32     | 9.59     | 10.11    | 10.39    | 14.15    | 9.04     | 19.43    | baseline                     |
| tsv-json          | 56.25      | 254  | 17.80    | 18.09    | 18.32    | 20.14    | 20.76    | 17.02    | 21.22    | 0.53x                        |
| tsv_wasm-json     | 54.56      | 221  | 18.31    | 18.66    | 20.06    | 20.53    | 24.91    | 18.04    | 25.45    | 0.51x                        |
| tsv-internal      | 297.81     | 1294 | 3.35     | 3.39     | 3.44     | 3.47     | 3.50     | 3.32     | 3.57     | 2.80x                        |
| tsv_wasm-internal | 216.85     | 906  | 4.61     | 4.64     | 4.70     | 4.73     | 4.77     | 4.56     | 5.47     | 2.04x                        |
| postcss           | 100.30     | 480  | 9.91     | 10.16    | 10.48    | 10.69    | 11.12    | 9.68     | 13.05    | 0.94x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 35.6 MB/s, tsv-json 18.8 MB/s, tsv_wasm-json 18.3 MB/s, tsv-internal 99.7 MB/s, tsv_wasm-internal 72.6 MB/s, postcss 33.6 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.3x tsv-internal, tsv_wasm-json 4.0x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.82       | 10  | 547.10   | 556.05   | 558.74   | 558.75   | 558.76   | 537.41   | 558.76   | baseline              |
| tsv        | 143.55     | 639 | 6.96     | 7.02     | 7.11     | 7.16     | 7.28     | 6.89     | 10.45    | 78.7x                 |
| tsv_wasm   | 101.86     | 481 | 9.79     | 9.90     | 9.98     | 10.05    | 10.33    | 9.68     | 13.35    | 55.9x                 |
| oxfmt      | 58.38      | 287 | 17.12    | 17.53    | 17.85    | 18.14    | 19.10    | 15.81    | 21.25    | 32.0x                 |
| biome-wasm | 5.18       | 26  | 220.18   | 233.66   | 239.71   | 240.80   | 243.52   | 134.91   | 244.42   | 2.84x                 |
| malva-wasm | 21.93      | 108 | 45.48    | 45.95    | 46.41    | 46.91    | 47.58    | 44.97    | 47.98    | 12.0x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 48.0 MB/s, tsv_wasm 34.1 MB/s, oxfmt 19.5 MB/s, biome-wasm 1.7 MB/s, malva-wasm 7.3 MB/s

_Note: every `Nx` is speedup form — values > 1 mean self is faster. File counts come from the per-group `Files (intersection):` / `Coverage:` lines and the Comparisons table row labels._

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 825.5 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 927.5 KB | 364.0 KB | 0.4x | 0.4x |
| tsv_wasm | 2.5 MB | 917.2 KB | — | — |
| biome (wasm) | 44.6 MB | 11.1 MB | 18.1x | 12.1x |
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
| format svelte (773f) | **59.4x** prettier, **59.7x** oxfmt |
| format typescript (2503f) | **24.3x** prettier, **1.52x** oxfmt |
| format css (49f) | **78.7x** prettier, **2.46x** oxfmt |
| parse svelte (773f) | **2.23x** svelte/compiler, **1.69x** rsvelte-parse |
| parse typescript (2502f) | **1.59x** acorn-typescript, **0.64x** oxc-parser, **0.20x** yuku-parser, **0.83x** swc |
| parse css (49f) | **0.53x** svelte/compiler, **0.56x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **41.8x** prettier, **8.70x** biome-wasm |
| format typescript (2503f) | **17.7x** prettier, **5.73x** biome-wasm, **4.04x** dprint-wasm |
| format css (49f) | **55.9x** prettier, **19.7x** biome-wasm, **4.65x** malva-wasm |
| parse svelte (773f) | **2.10x** svelte/compiler |
| parse typescript (2502f) | **1.55x** acorn-typescript, **0.65x** oxc-parser-wasm, **0.16x** yuku-parser-wasm |
| parse css (49f) | **0.51x** svelte/compiler, **0.54x** postcss |

_`Nx` is speedup — self is N× faster than the named opponent. `(Mf)` is the self impl's iterated count (per-group intersection in default mode; per-impl success set in `BENCH_MODE=union`). Parse canonical: svelte/compiler for svelte + css, acorn-typescript for typescript — each named by its own row. Format groups include parse time — each formatter parses internally. oxfmt formats JS/TS natively; its css/svelte rows route through its bundled prettier (+ svelte plugin, with the embedded `<script>` formatted natively), so `tsv` vs `oxfmt` is native-vs-native on typescript only. oxc-parser (native and wasm) serializes the AST to JSON in Rust and deserializes it in JS — the same eager materialization as tsv-json/tsv_wasm-json, so these parse rows are apples-to-apples. yuku-parser (native and wasm) decodes a binary AST buffer into JS objects — also full eager materialization (verified: no lazy accessors survive, and the tree serializes to within 3 bytes of oxc-parser), but its `parse()` is lazy, so the bench reads `.program` to force it — an unforced row would report a throughput for a tree nobody built. swc parses to its own AST dialect (root `Module`, `span` rather than `loc`, `Ts`-prefixed kinds), so it carries the same payload disclosure oxc-parser does — the mechanism matches `tsv-json` (serialize, cross, materialize) while the tree it produces is neither tsv’s loc-bearing drop-in shape nor its span-only wire. rsvelte-parse returns a compact JSON string the caller parses — the identical mechanism `tsv-json` measures (same serialize + boundary + `JSON.parse` cost) and within ~1.5% of its payload on a real component, so it is the one third-party parse row matched to tsv on BOTH axes. Its `skipExpressionLoc` variant is deliberately not compared: that reduction is not tsv’s span-only wire. postcss is the JS parser behind prettier’s CSS printer, i.e. behind the `format/css` baseline — a JS-vs-native read like prettier’s own, not a same-tier one; it is the only third-party engine available on `parse/css`, since no Rust CSS parser exposes an AST to JS. malva-wasm is dprint’s CSS plugin running over the same `@dprint/formatter` wasm host as dprint-wasm — a same-tier wasm-vs-wasm read, and with biome-wasm the only other engine on `format/css`. tsv-internal/tsv_wasm-internal are parse-only (no JS materialization) and have no counterpart row — oxc always serializes to cross into JS (experimentalLazy is setup-dominated), and yuku still serializes to a binary buffer before its decode, so neither is the same tier._

_Consumer-side: for full `loc`, fetching the span-only `no-locations` wire and reconstructing `loc` in JS (`reconstruct_locations`, shipped in every parse-capable package) beats the full loc-bearing `tsv-json` wire end-to-end — ~1.7x faster reconstructing every node, ~2.2x loc-free (TypeScript, exact; measured by `diagnostics/reconstruct_vs_materialize.ts`). Pre-materializing `loc` in Rust is not optimal for JS consumers._

## Skipped Files

12 unique file+error combinations — Svelte 0, TypeScript 12, CSS 0.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 3
- parse/typescript: swc: 3
- parse/typescript: oxc-parser: 2
- parse/typescript: oxc-parser-wasm: 2
- parse/typescript: yuku-parser: 2
- parse/typescript: yuku-parser-wasm: 2
- format/typescript: oxfmt: 2

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
