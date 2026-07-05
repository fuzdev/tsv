# tsv conformance benchmark results (parse)

**Runtime:** node

**Corpus kind:** conformance — full fixtures-included corpus, parse groups only; per-tool Coverage lines only (coverage-only run — timed throughput skipped)

**Date:** 2026-07-05T15:40:47.791Z — tsv 0.1.0 (d7d873b2)

**Corpus:** 5355 Svelte (2.7 MB), 46708 TypeScript (78.5 MB), 22672 CSS (7.8 MB) — 74735 files, 89.1 MB total

**Sources:** ../zzz/src (325), ../fuz_app/src (664), ../fuz_blog/src (32), ../fuz_code/src (64), ../fuz_css/src (124), ../fuz_docs/src (64), ../fuz_gitops/src (203), ../fuz_mastodon/src (24), ../fuz_template/src (15), ../fuz_ui/src (215), ../fuz_util/src (144), ../gro/src (187), ../svelte-docinfo/src (297), ../tsv.fuz.dev/src (28), ../kit/packages/kit/src (318), ../svelte/packages/svelte/src (375), ../svelte.dev/apps/svelte.dev/src (135), ../svelte.dev/packages/repl/src (48), ../svelte.dev/packages/site-kit/src (65), ../prettier-plugin-svelte/test (309), ../prettier/tests/format/typescript (789), ../prettier/tests/format/js (1103), ../prettier/tests/format/css (228), ../prettier/tests/format/html (124), ../svelte/packages/svelte/tests (4432), benches/js/.cache/wpt_css (22310), benches/js/.cache/test262_files.json (42113)

**Versions:** svelte@5.56.1, acorn@8.16.0, acorn-typescript@1.0.10, prettier@3.9.0, prettier-plugin-svelte@4.1.1, oxc-parser@0.134.0, oxfmt@0.53.0, @biomejs/wasm-bundler@2.4.16

**Methodology:** Single-threaded — every implementation formats/parses one file at a time, measured sequentially with no cross-file parallelism. The numbers are per-file, single-core latency/throughput, not the multi-core batch throughput a CLI gets formatting many files at once.

## parse/svelte

**Coverage:** svelte/compiler 5217/5355 (97%), tsv-json 5241/5355 (97%), tsv_wasm-json 5241/5355 (97%), tsv-internal 5241/5355 (97%), tsv_wasm-internal 5241/5355 (97%)

## parse/typescript

**Coverage:** acorn-typescript 46030/46708 (98%), tsv-json 46464/46708 (99%), tsv_wasm-json 46464/46708 (99%), tsv-internal 46464/46708 (99%), tsv_wasm-internal 46464/46708 (99%), oxc-parser 46465/46708 (99%), oxc-parser-wasm 46708/46708 (100%)

## parse/css

**Coverage:** svelte/compiler 22433/22672 (98%), tsv-json 22478/22672 (99%), tsv_wasm-json 22478/22672 (99%), tsv-internal 22478/22672 (99%), tsv_wasm-internal 22478/22672 (99%)

## Binary Sizes

| Binary | Size | Gzipped | vs tsv | vs tsv (gz) |
| --- | ---: | ---: | ---: | ---: |
| tsv_format_wasm | 2.2 MB | 754.3 KB | 0.9x | 0.9x |
| tsv_parse_wasm | 1.0 MB | 375.8 KB | 0.4x | 0.5x |
| tsv_wasm | 2.4 MB | 833.6 KB | — | — |
| biome (wasm) | 34.4 MB | 8.2 MB | 14.3x | 9.8x |
| oxc-parser (wasm) | 1.9 MB | 518.7 KB | 0.8x | 0.6x |
| tsv (ffi) | 3.3 MB | 1.4 MB | 1.0x | 1.0x |
| tsv format (ffi) | 3.1 MB | 1.3 MB | 0.9x | 0.9x |
| tsv parse (ffi) | 1.5 MB | 682.9 KB | 0.4x | 0.5x |
| tsv (napi) | 3.4 MB | 1.5 MB | — | — |
| oxc-parser+oxfmt (napi) | 10.7 MB | 4.3 MB | 3.1x | 2.9x |
| oxc-parser (napi) | 2.7 MB | 1.0 MB | 0.8x | 0.7x |
| oxfmt (napi) | 8.0 MB | 3.2 MB | 2.3x | 2.2x |

_Gzipped ≈ npm-tarball wire size (`gzip -c`, system default level). `vs tsv (gz)` compares gzipped bytes; `vs tsv` compares raw on-disk bytes._

## Skipped Files

1850 unique file+error combinations — Svelte 252, TypeScript 1165, CSS 433.

**Per-benchmark skip counts:**
- parse/typescript: acorn-typescript: 678
- parse/typescript: tsv-json: 244
- parse/typescript: tsv_wasm-json: 244
- parse/typescript: tsv-internal: 244
- parse/typescript: tsv_wasm-internal: 244
- parse/typescript: oxc-parser: 243
- parse/css: svelte/compiler: 239
- parse/css: tsv-json: 194
- parse/css: tsv_wasm-json: 194
- parse/css: tsv-internal: 194
- parse/css: tsv_wasm-internal: 194
- parse/svelte: svelte/compiler: 138
- parse/svelte: tsv-json: 114
- parse/svelte: tsv_wasm-json: 114
- parse/svelte: tsv-internal: 114
- parse/svelte: tsv_wasm-internal: 114

_Per-file detail omitted. Re-run with `--verbose` to include error messages and failure sets per file._
