# tsv benchmark results

**Runtime:** deno

**Machine:** AMD Ryzen 5 PRO 7530U with Radeon Graphics · linux/x86_64 · deno 2.9.4

**Corpus kind:** perf — real-world code only (fixture suites excluded)

**Date:** 2026-08-20T02:38:59.613Z — tsv 0.2.0 (8a87d997)

**Corpus:** 773 Svelte (2.0 MB), 2505 TypeScript (18.0 MB), 49 CSS (0.3 MB) — 3327 files, 20.3 MB total

**Sources:** ../zzz/src (326), ../fuz_app/src (671), ../fuz_blog/src (38), ../fuz_code/src (66), ../fuz_css/src (167), ../fuz_docs/src (65), ../fuz_gitops/src (99), ../fuz_mastodon/src (25), ../fuz_template/src (18), ../fuz_ui/src (233), ../fuz_util/src (147), ../mdz/src (69), ../gro/src (161), ../svelte-docinfo/src (119), ../tsv.fuz.dev/src (33), ../ryanatkn.com/src (52), ../webdevladder.net/src (39), benches/js/.cache/svelte_styles (18), ../kit/packages/kit/src (298), ../svelte/packages/svelte/src (415), ../svelte.dev/apps/svelte.dev/src (145), ../svelte.dev/packages/repl/src (53), ../svelte.dev/packages/site-kit/src (70)

**Versions:** svelte@5.56.9, acorn@8.16.0, acorn-typescript@1.0.13, prettier@3.9.6, prettier-plugin-svelte@4.1.1, oxc-parser@0.142.0, oxfmt@0.63.0, yuku-parser@0.8.7, @biomejs/wasm-bundler@2.5.8, @dprint/typescript@0.96.1, dprint-plugin-malva@0.16.0, postcss@8.5.26, @rsvelte/fmt@0.7.11, @rsvelte/vite-plugin-svelte-native@0.3.7 (targets svelte@5.56.9), @swc/core@1.16.0

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. One timed iteration is one full sweep over the group’s iterated file set, so the absolute columns (sweeps/sec, p50–p99, min/max) are per-sweep, not per-file — divide by the group’s file count (the Files lines / `(Mf)` annotations) for per-file figures; ratios and MB/s are denominated consistently either way. This is single-core throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

| Task Name                   | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| --------------------------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler             | 2.06       | 11  | 482.36   | 493.17   | 509.78   | 512.30   | 514.32   | 464.73   | 514.82   | baseline                     |
| tsv-json                    | 4.96       | 21  | 201.82   | 202.33   | 206.89   | 207.82   | 212.71   | 200.51   | 214.20   | 2.40x                        |
| tsv_wasm-json               | 4.26       | 20  | 234.62   | 235.82   | 238.65   | 239.34   | 240.42   | 233.22   | 240.69   | 2.06x                        |
| tsv-json-no-locations       | 7.80       | 37  | 127.57   | 129.47   | 131.34   | 131.76   | 132.96   | 126.58   | 133.10   | 3.78x                        |
| tsv_wasm-json-no-locations  | 6.36       | 30  | 156.91   | 157.80   | 160.84   | 161.27   | 161.79   | 155.67   | 161.81   | 3.08x                        |
| tsv-internal                | 51.42      | 188 | 19.42    | 19.87    | 20.11    | 20.36    | 20.88    | 19.33    | 21.18    | 24.9x                        |
| tsv_wasm-internal           | 32.64      | 131 | 30.56    | 31.20    | 31.38    | 31.48    | 31.66    | 30.37    | 34.99    | 15.8x                        |
| rsvelte-parse               | 2.83       | 15  | 352.88   | 355.18   | 357.24   | 358.82   | 360.46   | 348.94   | 360.87   | 1.37x                        |
| rsvelte-parse-skip-expr-loc | 4.80       | 23  | 208.20   | 209.64   | 213.08   | 214.99   | 215.68   | 205.59   | 215.80   | 2.33x                        |

**Files (intersection):** 773

**Throughput:** svelte/compiler 4.0 MB/s, tsv-json 9.7 MB/s, tsv_wasm-json 8.3 MB/s, tsv-json-no-locations 15.3 MB/s, tsv_wasm-json-no-locations 12.5 MB/s, tsv-internal 100.8 MB/s, tsv_wasm-internal 64.0 MB/s, rsvelte-parse 5.5 MB/s, rsvelte-parse-skip-expr-loc 9.4 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 10.4x tsv-internal, tsv_wasm-json 7.7x tsv_wasm-internal

## format/svelte

| Task Name  | sweeps/sec | n  | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | -- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 0.23       | 7  | 4328.59  | 4427.54  | 4537.28  | —        | —        | 4246.11  | 4633.99  | baseline              |
| tsv        | 13.38      | 59 | 74.56    | 75.82    | 77.50    | 77.76    | 78.21    | 73.92    | 78.36    | 58.5x                 |
| tsv_wasm   | 8.39       | 39 | 118.70   | 120.36   | 122.02   | 123.09   | 123.64   | 117.79   | 123.74   | 36.7x                 |
| oxfmt      | 0.23       | 6  | 4394.93  | 4416.39  | 4533.15  | —        | —        | 4273.63  | 4703.00  | 1.00x                 |
| biome-wasm | 1.32       | 6  | 758.63   | 759.39   | 766.10   | —        | —        | 754.59   | 775.41   | 5.77x                 |

**Files (intersection):** 773

**Throughput:** prettier 0.4 MB/s, tsv 26.2 MB/s, tsv_wasm 16.5 MB/s, oxfmt 0.4 MB/s, biome-wasm 2.6 MB/s

**Coverage-only (not timed):** rsvelte-fmt 773/773 (100%) — no in-process API, so a timed row would measure process spawn rather than format work; these are accept rates, not speeds.

## parse/typescript

| Task Name                  | sweeps/sec | n  | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs acorn-typescript (speedup) |
| -------------------------- | ---------- | -- | ------- | ------- | ------- | ------- | ------- | ------- | ------- | ----------------------------- |
| acorn-typescript           | 0.31       | 4  | 3.20    | 3.21    | 3.21    | —       | —       | 3.17    | 3.21    | baseline                      |
| tsv-json                   | 0.53       | 5  | 1.87    | 1.88    | 1.89    | —       | —       | 1.87    | 1.89    | 1.70x                         |
| tsv_wasm-json              | 0.47       | 5  | 2.12    | 2.12    | 2.13    | —       | —       | 2.11    | 2.14    | 1.51x                         |
| tsv-json-no-locations      | 1.11       | 5  | 0.90    | 0.91    | 0.91    | —       | —       | 0.90    | 0.92    | 3.54x                         |
| tsv_wasm-json-no-locations | 0.92       | 4  | 1.09    | 1.09    | 1.09    | —       | —       | 1.09    | 1.09    | 2.94x                         |
| tsv-internal               | 7.68       | 30 | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 0.13    | 24.5x                         |
| tsv_wasm-internal          | 5.06       | 26 | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 0.20    | 16.2x                         |
| oxc-parser                 | 0.82       | 5  | 1.21    | 1.23    | 1.24    | —       | —       | 1.19    | 1.25    | 2.63x                         |
| oxc-parser-wasm            | 0.74       | 3  | 1.34    | 1.34    | 1.35    | —       | —       | 1.34    | 1.36    | 2.38x                         |
| yuku-parser                | 2.20       | 12 | 0.46    | 0.46    | 0.47    | 0.47    | 0.48    | 0.43    | 0.48    | 7.04x                         |
| yuku-parser-wasm           | 2.51       | 11 | 0.40    | 0.41    | 0.45    | 0.47    | 0.48    | 0.37    | 0.48    | 8.02x                         |
| swc                        | 0.63       | 5  | 1.59    | 1.61    | 1.61    | —       | —       | 1.59    | 1.61    | 2.00x                         |

**Files (intersection):** 2502

**Throughput:** acorn-typescript 5.6 MB/s, tsv-json 9.6 MB/s, tsv_wasm-json 8.5 MB/s, tsv-json-no-locations 19.9 MB/s, tsv_wasm-json-no-locations 16.5 MB/s, tsv-internal 137.9 MB/s, tsv_wasm-internal 90.9 MB/s, oxc-parser 14.8 MB/s, oxc-parser-wasm 13.4 MB/s, yuku-parser 39.5 MB/s, yuku-parser-wasm 45.0 MB/s, swc 11.2 MB/s

**Coverage:** acorn-typescript 2502/2505 (99%), tsv-json 2505/2505 (100%), tsv_wasm-json 2505/2505 (100%), tsv-json-no-locations 2505/2505 (100%), tsv_wasm-json-no-locations 2505/2505 (100%), tsv-internal 2505/2505 (100%), tsv_wasm-internal 2505/2505 (100%), oxc-parser 2503/2505 (99%), oxc-parser-wasm 2503/2505 (99%), yuku-parser 2503/2505 (99%), yuku-parser-wasm 2503/2505 (99%), swc 2502/2505 (99%)

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 14.4x tsv-internal, tsv_wasm-json 10.7x tsv_wasm-internal

## format/typescript

| Task Name   | sweeps/sec | n | p50 (s) | p75 (s) | p90 (s) | p95 (s) | p99 (s) | min (s) | max (s) | vs prettier (speedup) |
| ----------- | ---------- | - | ------- | ------- | ------- | ------- | ------- | ------- | ------- | --------------------- |
| prettier    | 0.08       | 6 | 12.76   | 12.92   | 13.05   | —       | —       | 12.70   | 13.21   | baseline              |
| tsv         | 1.79       | 7 | 0.56    | 0.56    | 0.57    | —       | —       | 0.56    | 0.57    | 22.8x                 |
| tsv_wasm    | 1.15       | 5 | 0.87    | 0.87    | 0.88    | —       | —       | 0.87    | 0.88    | 14.7x                 |
| oxfmt       | 1.17       | 4 | 0.85    | 0.86    | 0.86    | —       | —       | 0.85    | 0.87    | 15.0x                 |
| biome-wasm  | 0.23       | 5 | 4.31    | 4.31    | 4.31    | —       | —       | 4.30    | 4.31    | 2.97x                 |
| dprint-wasm | 0.28       | 5 | 3.62    | 3.63    | 3.63    | —       | —       | 3.61    | 3.63    | 3.53x                 |

**Files (intersection):** 2503

**Throughput:** prettier 1.4 MB/s, tsv 32.1 MB/s, tsv_wasm 20.6 MB/s, oxfmt 21.1 MB/s, biome-wasm 4.2 MB/s, dprint-wasm 5.0 MB/s

**Coverage:** prettier 2505/2505 (100%), tsv 2505/2505 (100%), tsv_wasm 2505/2505 (100%), oxfmt 2503/2505 (99%), biome-wasm 2505/2505 (100%), dprint-wasm 2505/2505 (100%)

## parse/css

| Task Name         | sweeps/sec | n    | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs svelte/compiler (speedup) |
| ----------------- | ---------- | ---- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | ---------------------------- |
| svelte/compiler   | 110.36     | 549  | 9.03     | 9.33     | 9.55     | 9.67     | 10.07    | 8.53     | 12.14    | baseline                     |
| tsv-json          | 65.20      | 296  | 15.26    | 15.58    | 15.90    | 16.77    | 17.32    | 15.03    | 19.22    | 0.59x                        |
| tsv_wasm-json     | 51.40      | 250  | 19.38    | 19.72    | 19.99    | 20.54    | 21.01    | 19.03    | 21.35    | 0.47x                        |
| tsv-internal      | 325.60     | 1503 | 3.07     | 3.09     | 3.13     | 3.15     | 3.19     | 3.03     | 3.25     | 2.95x                        |
| tsv_wasm-internal | 184.25     | 774  | 5.42     | 5.46     | 5.51     | 5.54     | 5.57     | 5.40     | 5.86     | 1.67x                        |
| postcss           | 99.09      | 491  | 10.04    | 10.28    | 10.60    | 10.78    | 11.20    | 9.69     | 13.15    | 0.90x                        |

**Files (intersection):** 49

**Throughput:** svelte/compiler 36.9 MB/s, tsv-json 21.8 MB/s, tsv_wasm-json 17.2 MB/s, tsv-internal 109.0 MB/s, tsv_wasm-internal 61.7 MB/s, postcss 33.2 MB/s

**JSON overhead** (json_ns / internal_ns, higher = more cost): tsv-json 5.0x tsv-internal, tsv_wasm-json 3.6x tsv_wasm-internal

## format/css

| Task Name  | sweeps/sec | n   | p50 (ms) | p75 (ms) | p90 (ms) | p95 (ms) | p99 (ms) | min (ms) | max (ms) | vs prettier (speedup) |
| ---------- | ---------- | --- | -------- | -------- | -------- | -------- | -------- | -------- | -------- | --------------------- |
| prettier   | 1.89       | 10  | 526.70   | 540.97   | 548.81   | 553.05   | 556.43   | 511.17   | 557.28   | baseline              |
| tsv        | 154.08     | 597 | 6.49     | 6.57     | 6.72     | 6.90     | 7.12     | 6.44     | 7.63     | 81.5x                 |
| tsv_wasm   | 85.95      | 336 | 11.62    | 11.81    | 11.92    | 11.96    | 12.01    | 11.55    | 13.13    | 45.5x                 |
| oxfmt      | 58.33      | 290 | 17.13    | 17.60    | 17.94    | 18.25    | 19.25    | 15.45    | 20.80    | 30.8x                 |
| biome-wasm | 9.99       | 44  | 99.92    | 101.05   | 102.61   | 103.07   | 104.16   | 99.11    | 104.32   | 5.28x                 |
| malva-wasm | 20.21      | 97  | 49.27    | 49.91    | 50.13    | 50.23    | 50.57    | 48.95    | 50.79    | 10.7x                 |

**Files (intersection):** 49

**Throughput:** prettier 0.6 MB/s, tsv 51.6 MB/s, tsv_wasm 28.8 MB/s, oxfmt 19.5 MB/s, biome-wasm 3.3 MB/s, malva-wasm 6.8 MB/s

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
| tsv (ffi) | 3.5 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 11.2 MB | 4.6 MB | 3.2x | 3.0x |
| tsv format (ffi) | 3.2 MB | 1.4 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 666.1 KB | 0.4x | 0.4x |
| tsv (napi) | 3.8 MB | 1.6 MB | 1.1x | 1.1x |
| oxc-parser (napi) | 2.1 MB | 885.7 KB | 0.6x | 0.6x |
| oxfmt (napi) | 9.0 MB | 3.7 MB | 2.6x | 2.4x |
| yuku-parser (napi) | 741.1 KB | 310.4 KB | 0.2x | 0.2x |
| rsvelte-fmt (binary) | 8.3 MB | 3.3 MB | 2.4x | 2.2x |
| rsvelte compiler (napi) | 14.5 MB | 6.0 MB | 4.2x | 4.0x |
| swc (napi) | 31.9 MB | 11.9 MB | 9.2x | 7.8x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Comparisons to tsv (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **58.5x** prettier, **58.3x** oxfmt |
| format typescript (2503f) | **22.8x** prettier, **1.52x** oxfmt |
| format css (49f) | **81.5x** prettier, **2.64x** oxfmt |
| parse svelte (773f) | **2.40x** svelte/compiler, **1.75x** rsvelte-parse |
| parse typescript (2502f) | **1.70x** acorn-typescript, **0.65x** oxc-parser, **0.24x** yuku-parser, **0.85x** swc |
| parse css (49f) | **0.59x** svelte/compiler, **0.66x** postcss |

## Comparisons to tsv_wasm (speedup)

| Benchmark | Comparisons |
| --- | --- |
| format svelte (773f) | **36.7x** prettier, **6.36x** biome-wasm |
| format typescript (2503f) | **14.7x** prettier, **4.94x** biome-wasm, **4.16x** dprint-wasm |
| format css (49f) | **45.5x** prettier, **8.61x** biome-wasm, **4.25x** malva-wasm |
| parse svelte (773f) | **2.06x** svelte/compiler |
| parse typescript (2502f) | **1.51x** acorn-typescript, **0.63x** oxc-parser-wasm, **0.19x** yuku-parser-wasm |
| parse css (49f) | **0.47x** svelte/compiler, **0.52x** postcss |

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
