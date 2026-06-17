# tsv changelog

Covers the npm packages published from this repo — `@fuzdev/tsv_format_wasm`,
`@fuzdev/tsv_parse_wasm`, and `@fuzdev/tsv_wasm`. All move together at the
`Cargo.toml [workspace.package]` version. Each `## Unreleased` section must be
non-empty and carry a `<!-- bump: patch|minor|major -->` marker; `deno task publish
--wetrun --bump <level>` requires `<level>` to match it, then stamps the section
(marker removed) into the released version's section and seeds a fresh empty
`## Unreleased` (reset to `bump: patch`) for the next cycle.

## Unreleased
<!-- bump: minor -->

- various conformance fixes to the formatter and parser
- numerous new Prettier divergences including uniform indentation on continuations
- reduce allocations using `SmallVec` and memoizations
- formatting is now **non-configurable by design** — no config files, CLI flags,
  or runtime options, and none are planned (opinionated like `gofmt` and Black).
  Policy only — no change to formatter output or the published API
  (`format_*` / `parse_*` / the `tsv` bin).
- parse AST: each comment now appears **once** in the public AST everywhere. tsv
  previously replicated acorn-typescript's backtrack-and-reparse comment
  duplication for type-space constructs (type literals, mapped/function types,
  type assertions, type-member index/computed signatures, typed-param arrows);
  it now corrects the duplication uniformly, matching the existing class-body
  behavior. The set of distinct comments is unchanged; only the duplicate
  entries are gone. Formatter output is unaffected.

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
