# tsv changelog

Covers the npm packages published from this repo — `@fuzdev/tsv_format_wasm`,
`@fuzdev/tsv_parse_wasm`, and `@fuzdev/tsv_wasm`, plus the native N-API set
(`@fuzdev/tsv` and its `@fuzdev/tsv-<triple>` platform packages). All move
together at the `Cargo.toml [workspace.package]` version.
Each `## Unreleased` section must be non-empty and carry a
`<!-- bump: patch|minor|major -->` marker; `deno task publish
--wetrun --bump <level>` requires `<level>` to match it, then stamps the section
(marker removed) into the released version's section and seeds a fresh empty
`## Unreleased` (reset to `bump: patch`) for the next cycle.

## Unreleased
<!-- bump: minor -->

- feat: publish native binaries, both CLI and JS lib — install `@fuzdev/tsv`
  (N-API addon plus the real native `tsv` CLI; `npx tsv` execs the binary), over
  `@fuzdev/tsv-<triple>` platform packages for Linux (x64 gnu and musl, arm64 gnu),
  macOS arm64, and Windows x64; API-compatible with `@fuzdev/tsv_wasm` (minus the
  WASM-only `init`/`wasm_module`/`reinstantiate`), which stays the universal fallback
  ([#717](https://github.com/fuzdev/tsv/pull/717),
  [#718](https://github.com/fuzdev/tsv/pull/718), [#720](https://github.com/fuzdev/tsv/pull/720),
  [#725](https://github.com/fuzdev/tsv/pull/725), [#726](https://github.com/fuzdev/tsv/pull/726))
- **breaking** chore: every package requires Node >=22
  ([#725](https://github.com/fuzdev/tsv/pull/725))
- **breaking** feat: every WASM parse/format export now takes an optional acorn-style
  options object — `parse_*(source, {locations?, goal?})` and `format_*(source, {goal?})`,
  replacing the flat `*_no_locations` / `*_with_goal` names; unknown keys throw, `goal` is
  TypeScript-only ([#645](https://github.com/fuzdev/tsv/pull/645),
  [#713](https://github.com/fuzdev/tsv/pull/713))
- **breaking** fix: parse output tracks its canonical oracles — CSS roots gain
  `comments: CSSComment[]` and `::part()`/`::slotted()` gain an `args: SelectorList`
  ([#766](https://github.com/fuzdev/tsv/pull/766)), Svelte components without `lang="ts"`
  emit vanilla acorn's wire shape in every expression island, not just `<script>` —
  `ImportExpression.options` instead of `arguments`, get/set `Property` key order
  ([#774](https://github.com/fuzdev/tsv/pull/774)), acorn-typescript alignments through
  1.0.13 ([#702](https://github.com/fuzdev/tsv/pull/702),
  [#764](https://github.com/fuzdev/tsv/pull/764)), plus span, loc and key-order alignments
  ([#553](https://github.com/fuzdev/tsv/pull/553), [#583](https://github.com/fuzdev/tsv/pull/583),
  [#625](https://github.com/fuzdev/tsv/pull/625), [#885](https://github.com/fuzdev/tsv/pull/885))
- fix: the bundled `tsv_ast.d.ts` now types the whole wire and is gated against the fixture
  corpus — `Script.content` is a `Program` (was `unknown`), `LogicalExpression` and the
  Svelte-built node shapes are declared, acorn nodes inside a Svelte component carry
  `leadingComments`/`trailingComments`, and the never-emitted `TSInterfaceHeritage` is gone
  ([#890](https://github.com/fuzdev/tsv/pull/890))
- **breaking** fix: naming a file tsv doesn't format (`tsv format some.json`) is now an
  upfront argument error instead of being parsed as TypeScript; directory arguments are
  unaffected ([#709](https://github.com/fuzdev/tsv/pull/709))
- feat: more formatting that takes advantage of Svelte 5 whitespace changes
  ([#558](https://github.com/fuzdev/tsv/pull/558), [#563](https://github.com/fuzdev/tsv/pull/563),
  [#600](https://github.com/fuzdev/tsv/pull/600), [#601](https://github.com/fuzdev/tsv/pull/601),
  [#606](https://github.com/fuzdev/tsv/pull/606), [#607](https://github.com/fuzdev/tsv/pull/607),
  [#609](https://github.com/fuzdev/tsv/pull/609), [#750](https://github.com/fuzdev/tsv/pull/750))
- fix: many formatting fixes, and new standing audits — several cleared to 0, comment
  injection among them
- feat: the `@fuzdev/tsv_wasm` `tsv` bin formats directories in parallel on
  `node:worker_threads` — `--jobs` is no longer ignored
  ([#873](https://github.com/fuzdev/tsv/pull/873))
- fix: a panic no longer breaks the engine — the WASM instance stays callable after a trap
  ([#616](https://github.com/fuzdev/tsv/pull/616)) and the native addon throws a JS error
  instead of aborting the host ([#717](https://github.com/fuzdev/tsv/pull/717)); the one
  trap that does poison a WASM instance, a stack overflow (~1,600 nesting levels), is now
  recoverable — the new `reinstantiate()` export swaps in a fresh instance synchronously
  with no recompile (`free()` old `IgnoreStack`s first), and the `tsv` bin calls it
  automatically, so one too-deep file is one error, not every file after it
  ([#886](https://github.com/fuzdev/tsv/pull/886))
- fix: every `tsv` CLI route and thread runs on one stated 32 MiB stack (~26,900 nesting
  levels) ([#874](https://github.com/fuzdev/tsv/pull/874))
- fix: an explicit `--jobs` past `4 × logical CPUs` clamps with a warning, and a thread the
  OS refuses narrows the pool instead of failing the run — both CLIs, native and JS
  ([#883](https://github.com/fuzdev/tsv/pull/883), [#884](https://github.com/fuzdev/tsv/pull/884))
- fix: every `<CR>`/`<CR><LF>` in formatted output folds to `<LF>`, verbatim-copied
  regions included ([#880](https://github.com/fuzdev/tsv/pull/880))
- feat: `tsv --version` ([#726](https://github.com/fuzdev/tsv/pull/726))

## 0.2.0

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
  [#540](https://github.com/fuzdev/tsv/pull/540), [#541](https://github.com/fuzdev/tsv/pull/541),
  [#542](https://github.com/fuzdev/tsv/pull/542))

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
