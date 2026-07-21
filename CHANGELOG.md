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
tsv has no config that changes its formatting style behavior, and none will be added.
(this has no observable API changes because options had been deferred)

- feat: adopt Svelte's Prettier settings,
  `bracketSpacing: true` and `trailingComma: 'none'`
  [#78](https://github.com/fuzdev/tsv/pull/78)
- feat: collapse render-insignificant spaces and
  converge on block style wrapping using Svelte 5 whitespace changes
  ([#76](https://github.com/fuzdev/tsv/pull/76), [#447](https://github.com/fuzdev/tsv/pull/447),
  [#449](https://github.com/fuzdev/tsv/pull/449), [#515](https://github.com/fuzdev/tsv/pull/515))
- feat: `tsv format` directory discovery now honors `.gitignore` and the tsv-native
  `.formatignore` hierarchically (one per directory, repo-rooted like git —
  unlike Prettier, which reads only one `.gitignore` and one `.prettierignore`
  relative to the cwd), plus a repo-root `.prettierignore` for drop-in compat
  ([#50](https://github.com/fuzdev/tsv/pull/50))
- feat: `tsv format --list` prints the in-scope files without formatting
- feat: support `format-ignore` as an alias to `prettier-ignore`  
  (along with `format-ignore-start` and `format-ignore-end` for templates)
  ([#41](https://github.com/fuzdev/tsv/pull/41))
- fix: various conformance fixes to the formatter and parser
- feat: uniform indentation on continuations
  ([#27](https://github.com/fuzdev/tsv/pull/27), [#33](https://github.com/fuzdev/tsv/pull/33))
- fix: expressions in Svelte block tags now consistently use TS printing paths,
  fixing oversights prettier-plugin-svelte
- test: add seeded mutational fuzzer
- perf: avoid Token copying in lexer [#191](https://github.com/fuzdev/tsv/pull/191)
- perf: reduce heap allocations
  ([#17](https://github.com/fuzdev/tsv/pull/17), [#19](https://github.com/fuzdev/tsv/pull/19),
  [#20](https://github.com/fuzdev/tsv/pull/20), [#23](https://github.com/fuzdev/tsv/pull/23),
  [#108](https://github.com/fuzdev/tsv/pull/108), [#111](https://github.com/fuzdev/tsv/pull/111),
  [#115](https://github.com/fuzdev/tsv/pull/115), [#116](https://github.com/fuzdev/tsv/pull/116),
  [#120](https://github.com/fuzdev/tsv/pull/120), [#121](https://github.com/fuzdev/tsv/pull/121),
  [#124](https://github.com/fuzdev/tsv/pull/124), [#128](https://github.com/fuzdev/tsv/pull/128),
  [#130](https://github.com/fuzdev/tsv/pull/130), [#132](https://github.com/fuzdev/tsv/pull/132),
  [#137](https://github.com/fuzdev/tsv/pull/137), [#143](https://github.com/fuzdev/tsv/pull/143),
  [#145](https://github.com/fuzdev/tsv/pull/145), [#147](https://github.com/fuzdev/tsv/pull/147),
  [#148](https://github.com/fuzdev/tsv/pull/148), [#151](https://github.com/fuzdev/tsv/pull/151),
  [#156](https://github.com/fuzdev/tsv/pull/156), [#165](https://github.com/fuzdev/tsv/pull/165),
  [#199](https://github.com/fuzdev/tsv/pull/199), [#200](https://github.com/fuzdev/tsv/pull/200),
  [#205](https://github.com/fuzdev/tsv/pull/205), [#208](https://github.com/fuzdev/tsv/pull/208),
  [#209](https://github.com/fuzdev/tsv/pull/209), [#210](https://github.com/fuzdev/tsv/pull/210),
  [#211](https://github.com/fuzdev/tsv/pull/211), [#212](https://github.com/fuzdev/tsv/pull/212),
  [#215](https://github.com/fuzdev/tsv/pull/215), [#220](https://github.com/fuzdev/tsv/pull/220),
  [#221](https://github.com/fuzdev/tsv/pull/221), [#231](https://github.com/fuzdev/tsv/pull/231),
  [#250](https://github.com/fuzdev/tsv/pull/250), [#254](https://github.com/fuzdev/tsv/pull/254),
  [#290](https://github.com/fuzdev/tsv/pull/290), [#292](https://github.com/fuzdev/tsv/pull/292),
  [#300](https://github.com/fuzdev/tsv/pull/300), [#305](https://github.com/fuzdev/tsv/pull/305),
  [#308](https://github.com/fuzdev/tsv/pull/308), [#309](https://github.com/fuzdev/tsv/pull/309),
  [#537](https://github.com/fuzdev/tsv/pull/537), [#539](https://github.com/fuzdev/tsv/pull/539),
  [#540](https://github.com/fuzdev/tsv/pull/540), [#541](https://github.com/fuzdev/tsv/pull/541))

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
