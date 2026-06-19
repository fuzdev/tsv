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

Formatting is now non-configurable by design -
tsv has no config that changes its formatting behavior, and none will be added.
(this has no observable API changes because options had been deferred)

- feat: support `format-ignore` as an alias to `prettier-ignore`
  (along with `format-ignore-start` and `format-ignore-end` for templates)
  ([#41](https://github.com/fuzdev/tsv/pull/41))
- fix: various conformance fixes to the formatter and parser
- fix: numerous new Prettier divergences including more readable block structure layouts
  and uniform indentation on continuations
- fix: expressions in Svelte block tags now consistently use TS printing paths,
  fixing oversights prettier-plugin-svelte
- perf: reduce allocations using `SmallVec` and memoizations
  ([#17](https://github.com/fuzdev/tsv/pull/17), [#19](https://github.com/fuzdev/tsv/pull/19),
  [#20](https://github.com/fuzdev/tsv/pull/20), [#23](https://github.com/fuzdev/tsv/pull/23))

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
