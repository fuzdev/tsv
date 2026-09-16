# tsv changelog

Covers the npm packages published from this repo — `@fuzdev/tsv-format-wasm`,
`@fuzdev/tsv-parse-wasm`, and `@fuzdev/tsv-wasm`, plus the native N-API set
(`@fuzdev/tsv` and its `@fuzdev/tsv-<triple>` platform packages). All move
together at the `Cargo.toml [workspace.package]` version.
Each `## Unreleased` section must be non-empty and carry a
`<!-- bump: patch|minor|major -->` marker; `deno task publish
--wetrun --bump <level>` requires `<level>` to match it, then stamps the section
(marker removed) into the released version's section and seeds a fresh empty
`## Unreleased` (reset to `bump: patch`) for the next cycle.

## Unreleased
<!-- bump: patch -->

## 0.4.0

- **breaking** chore: the three WASM packages are renamed to kebab-case — `@fuzdev/tsv_wasm` →
  `@fuzdev/tsv-wasm`, `@fuzdev/tsv_format_wasm` → `@fuzdev/tsv-format-wasm`,
  `@fuzdev/tsv_parse_wasm` → `@fuzdev/tsv-parse-wasm` — matching the `@fuzdev/tsv-<triple>` platform
  packages, the `tsv-<triple>` release assets and the `tsv-format` VS Code extension, so tsv's whole
  npm surface spells one way; the snake_case names stop at 0.3.x and are deprecated on npm with a
  pointer to their replacement. Exports, options and the `tsv` bin are unchanged; the Rust crates
  keep their `tsv_*` names
- feat: a `darwin-x64` platform package joins the native set, so `@fuzdev/tsv` installs on Intel
  Macs and under x64 Node on Apple Silicon instead of throwing at import; its `tsv` CLI binary is
  attached to the GitHub Release like the others
  ([#1040](https://github.com/fuzdev/tsv/pull/1040))
- fix: a relational chain such as `x < y > z` no longer prints in a form that re-parses as a
  type-argument instantiation when a line break lands after the `>` — tsv keeps a paren pair around
  the `<` operand, so the output reads back as the comparison the author wrote
  ([#1039](https://github.com/fuzdev/tsv/pull/1039))
- fix: restore the CSS parse speed lost in 0.3.0 — a declaration value is ruled out for operator
  splitting before its run is tokenized ([#1037](https://github.com/fuzdev/tsv/pull/1037))

## 0.3.0

- **breaking** feat: native binaries, both CLI and JS lib — install `@fuzdev/tsv` (N-API addon plus
  the real native `tsv` CLI; `npx tsv` execs the binary) over `@fuzdev/tsv-<triple>` platform
  packages for Linux (x64 gnu and musl, arm64 gnu), macOS arm64, and Windows x64; API-compatible
  with `@fuzdev/tsv_wasm` (minus the WASM-only `init`/`init_sync`/`wasm_module`/`reinstantiate` and
  `IgnoreStack`'s `free()`), which stays the universal fallback. The same CLI binaries are attached
  to each GitHub Release with a `SHA256SUMS`, every asset carrying a Sigstore build provenance
  attestation, for use without npm
  ([#717](https://github.com/fuzdev/tsv/pull/717), [#718](https://github.com/fuzdev/tsv/pull/718),
  [#720](https://github.com/fuzdev/tsv/pull/720), [#725](https://github.com/fuzdev/tsv/pull/725),
  [#726](https://github.com/fuzdev/tsv/pull/726), [#1035](https://github.com/fuzdev/tsv/pull/1035))
- **breaking** chore: every package now requires Node >=22 (was >=20)
  ([#725](https://github.com/fuzdev/tsv/pull/725))
- **breaking** feat: every parse/format export takes an optional acorn-style options object —
  `parse_*(source, {locations?, sourceType?})` and `format_*(source, {sourceType?})`, replacing the
  flat `*_no_locations` / `*_with_goal` names; unknown keys throw, `sourceType` is TypeScript-only,
  and `ParseOptions`/`FormatOptions` (plus their `TypeScript*` variants) are exported types. The CLI
  flag is `--source-type` (was `--goal`), and naming one for a Svelte or CSS input is an error rather
  than ignored ([#645](https://github.com/fuzdev/tsv/pull/645),
  [#713](https://github.com/fuzdev/tsv/pull/713))
- **breaking** feat: the exported `IgnoreStack` splits `push_tsv` into `push_formatignore` /
  `push_prettierignore`, `heuristic_shadow_warning` becomes `shadow_warning` (returning `undefined`
  when there's nothing to warn), and it gains the per-path queries the CLIs use —
  `path_shadow_warning`, `unsupported_extension_error`, `excluded_argument_warning`,
  `gitignore_symlink_warning`, `unresolvable_root_error` — so an embedder reproduces exactly what
  `tsv format` would touch
- **breaking** feat: strictness follows the spec — `sourceType: 'script'` parses a sloppy script
  (strict only under its own `"use strict"` prologue, so `with` and legacy octal literals and
  escapes parse there), a module is always strict (legacy string escapes now reject in one), and
  `format` with no `sourceType` parses as a module and retries as a script only if that fails, so
  legacy scripts format from a bare `tsv format` (`.mjs`/`.mts` excepted); `with` is a reserved word
  in every mode, so it no longer parses as a name
- **breaking** fix: the parsers reject what their oracles reject — TypeScript: a raw newline inside
  a string literal, the `[no LineTerminator here]` positions, `for await` without `of`, a definite
  `!` on a `for` header's declaration, `typeof 5`, a compound operator inside a destructuring or
  `for` target (`[a += b] = xs`, which used to parse and drop the operator); Svelte: a duplicate
  `{:else}`, a block tag in attribute position, an unterminated quoted attribute (each of which used
  to parse and corrupt the document), an unknown `<svelte:*>` name, a nested `<svelte:options>` —
  and accept what they accept: strict-mode-reserved words as binding names (`let`, `yield`,
  `implements`), `let` as a reference in `for` heads, type-asserted `for-in`/`of` targets
  (`for (a! of xs)`), decorator type arguments, ambient generator/async declarations,
  `declare module 'x'` with no `;`, and the tsc corpus's remaining over-rejections
  ([#618](https://github.com/fuzdev/tsv/pull/618), [#619](https://github.com/fuzdev/tsv/pull/619),
  [#621](https://github.com/fuzdev/tsv/pull/621), [#628](https://github.com/fuzdev/tsv/pull/628),
  [#629](https://github.com/fuzdev/tsv/pull/629), [#630](https://github.com/fuzdev/tsv/pull/630),
  [#638](https://github.com/fuzdev/tsv/pull/638), [#640](https://github.com/fuzdev/tsv/pull/640),
  [#673](https://github.com/fuzdev/tsv/pull/673), [#705](https://github.com/fuzdev/tsv/pull/705),
  [#708](https://github.com/fuzdev/tsv/pull/708), [#710](https://github.com/fuzdev/tsv/pull/710),
  [#711](https://github.com/fuzdev/tsv/pull/711), [#736](https://github.com/fuzdev/tsv/pull/736),
  [#737](https://github.com/fuzdev/tsv/pull/737), [#776](https://github.com/fuzdev/tsv/pull/776),
  [#778](https://github.com/fuzdev/tsv/pull/778), [#812](https://github.com/fuzdev/tsv/pull/812),
  [#814](https://github.com/fuzdev/tsv/pull/814), [#872](https://github.com/fuzdev/tsv/pull/872),
  [#877](https://github.com/fuzdev/tsv/pull/877), [#878](https://github.com/fuzdev/tsv/pull/878),
  [#881](https://github.com/fuzdev/tsv/pull/881), [#893](https://github.com/fuzdev/tsv/pull/893),
  [#894](https://github.com/fuzdev/tsv/pull/894), [#895](https://github.com/fuzdev/tsv/pull/895),
  [#912](https://github.com/fuzdev/tsv/pull/912), [#915](https://github.com/fuzdev/tsv/pull/915))
- **breaking** fix: parse output tracks its canonical oracles — CSS roots gain `comments:
  CSSComment[]` and `::part()`/`::slotted()` gain an `args: SelectorList`
  ([#766](https://github.com/fuzdev/tsv/pull/766)), Svelte components without `lang="ts"` emit
  vanilla acorn's wire shape in every expression island, not just `<script>` —
  `ImportExpression.options` instead of `arguments`, get/set `Property` key order
  ([#774](https://github.com/fuzdev/tsv/pull/774)) — comment attachment reaches every island node
  ([#891](https://github.com/fuzdev/tsv/pull/891), [#896](https://github.com/fuzdev/tsv/pull/896)),
  acorn-typescript alignments through 1.0.13 ([#702](https://github.com/fuzdev/tsv/pull/702),
  [#764](https://github.com/fuzdev/tsv/pull/764)), span, loc and key-order alignments
  ([#583](https://github.com/fuzdev/tsv/pull/583), [#625](https://github.com/fuzdev/tsv/pull/625),
  [#882](https://github.com/fuzdev/tsv/pull/882), [#885](https://github.com/fuzdev/tsv/pull/885),
  [#888](https://github.com/fuzdev/tsv/pull/888), [#889](https://github.com/fuzdev/tsv/pull/889),
  [#919](https://github.com/fuzdev/tsv/pull/919)), Svelte and CSS offsets counted past a leading
  BOM as their parsers strip it (TypeScript keeps file coordinates, as acorn does), a template
  element's `raw`/`cooked` folding a literal `<CR><LF>` to `<LF>` per the spec
  ([#727](https://github.com/fuzdev/tsv/pull/727)), JS-exact numeric literal values
  ([#553](https://github.com/fuzdev/tsv/pull/553)), and corrected legacy-octal-escape and HTML
  character-reference decoding ([#633](https://github.com/fuzdev/tsv/pull/633),
  [#635](https://github.com/fuzdev/tsv/pull/635))
- fix: the bundled `tsv_ast.d.ts` now types the whole wire outside the CSS subtree and is gated
  against the fixture corpus — `Script.content` is a `Program` (was `unknown`),
  `Program.sourceType` is `'script' | 'module'`, `LogicalExpression`, `WithStatement` and the
  Svelte-built node shapes are declared, acorn nodes inside a Svelte component carry
  `leadingComments`/`trailingComments`, and the never-emitted `TSInterfaceHeritage` and
  `TSMappedTypeParameter` are gone
  ([#890](https://github.com/fuzdev/tsv/pull/890))
- **breaking** fix: a path named on the command line is bounded by the ignore files like a walked
  one — a file or directory a `.gitignore` rule excludes is skipped with a warning that gives the
  re-include lines, one a `.formatignore`/`.prettierignore` rule excludes is skipped quietly (as
  prettier does), and a run whose every argument was an excluded file exits 0; a symlinked argument
  is graded as the link git sees, not its target. Naming a file tsv doesn't format
  (`tsv format some.json`) is now an upfront argument error instead of being parsed as TypeScript
  ([#709](https://github.com/fuzdev/tsv/pull/709))
- **breaking** feat: `format-ignore` / `prettier-ignore` are honored in many more positions (type
  members, parameters, arguments, declarators, statement and declaration heads, Svelte braced
  heads), and only an own-line directive acts — one sharing its line with code is an ordinary
  comment ([#572](https://github.com/fuzdev/tsv/pull/572),
  [#913](https://github.com/fuzdev/tsv/pull/913)); a frozen statement keeps its ASI semicolon and
  required parens so the output reparses
  ([#897](https://github.com/fuzdev/tsv/pull/897)), and a `-start`/`-end` range no longer prints a
  `<script>`/`<style>` inside it twice ([#610](https://github.com/fuzdev/tsv/pull/610))
- feat: more formatting that takes advantage of Svelte 5 whitespace changes
  ([#558](https://github.com/fuzdev/tsv/pull/558), [#563](https://github.com/fuzdev/tsv/pull/563),
  [#600](https://github.com/fuzdev/tsv/pull/600), [#601](https://github.com/fuzdev/tsv/pull/601),
  [#606](https://github.com/fuzdev/tsv/pull/606), [#607](https://github.com/fuzdev/tsv/pull/607),
  [#609](https://github.com/fuzdev/tsv/pull/609), [#750](https://github.com/fuzdev/tsv/pull/750),
  [#768](https://github.com/fuzdev/tsv/pull/768), [#905](https://github.com/fuzdev/tsv/pull/905),
  [#906](https://github.com/fuzdev/tsv/pull/906), [#907](https://github.com/fuzdev/tsv/pull/907),
  [#908](https://github.com/fuzdev/tsv/pull/908), [#909](https://github.com/fuzdev/tsv/pull/909),
  [#910](https://github.com/fuzdev/tsv/pull/910), [#911](https://github.com/fuzdev/tsv/pull/911),
  [#918](https://github.com/fuzdev/tsv/pull/918), [#1006](https://github.com/fuzdev/tsv/pull/1006))
- fix: many formatting fixes
- fix: no comment is dropped or double-printed, and a `//` no longer swallows the code after it
  into an output that doesn't reparse; an embedded body in a language tsv doesn't format
  (`<script lang="coffee">`, `<template lang="pug">`, `type="application/json"`) is copied verbatim
  instead of being parsed or re-indented ([#875](https://github.com/fuzdev/tsv/pull/875),
  [#879](https://github.com/fuzdev/tsv/pull/879))
- feat: the JS CLIs format directories in parallel on `node:worker_threads` — `--jobs` is no longer
  ignored. The Node entry exports `wasm_module` and the `./worker` subpath initializes a worker from
  it without recompiling. On every CLI, an explicit `--jobs` past `4 × logical CPUs` clamps with a
  warning and a thread the OS refuses narrows the pool instead of failing the run
  ([#873](https://github.com/fuzdev/tsv/pull/873), [#883](https://github.com/fuzdev/tsv/pull/883),
  [#884](https://github.com/fuzdev/tsv/pull/884))
- fix: both CLIs survive a closed or non-blocking pipe (`tsv format . | head` no longer aborts or
  loses its report of what changed), and the JS CLI refuses to rewrite a file whose bytes aren't
  valid UTF-8 rather than writing U+FFFD back over it
- fix: a panic no longer breaks the engine — the WASM instance stays callable after a trap
  ([#616](https://github.com/fuzdev/tsv/pull/616)) and the native addon throws a JS error instead of
  aborting the host ([#717](https://github.com/fuzdev/tsv/pull/717)); the one trap that does poison
  a WASM instance, a stack overflow (~2,500 nesting levels), is recoverable through the new
  `reinstantiate()` export (objects from the old instance are invalidated; `free()` on one is a safe
  no-op), which the `tsv` bin calls automatically ([#886](https://github.com/fuzdev/tsv/pull/886));
  every native `tsv` CLI route and thread runs on one stated 32 MiB stack
  ([#874](https://github.com/fuzdev/tsv/pull/874))
- fix: every `<CR>`/`<CR><LF>` in formatted output folds to `<LF>`, verbatim-copied regions included
  ([#880](https://github.com/fuzdev/tsv/pull/880)); `tsv parse --pretty` no longer fails on deeply
  nested input ([#1001](https://github.com/fuzdev/tsv/pull/1001)); the CSS printer keeps a comma
  closing a declaration value (`transition: a,`) rather than dropping it
  ([#615](https://github.com/fuzdev/tsv/pull/615))
- feat: `tsv --version` ([#726](https://github.com/fuzdev/tsv/pull/726)); the `locations` helper
  rebuilds Svelte `name_loc` ([#582](https://github.com/fuzdev/tsv/pull/582)) and throws, rather
  than guessing, on the two Svelte shapes the span-only wire can't disambiguate
  ([#885](https://github.com/fuzdev/tsv/pull/885)); `.d.ts` relative specifiers carry `.js`, so the
  packages type under `moduleResolution: node16`/`nodenext`
  ([#873](https://github.com/fuzdev/tsv/pull/873))

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
- perf: reduce heap allocations across the lexer, parser and printers (~47 PRs, #17–#542)

## 0.1.0

- init
- add `@fuzdev/tsv_wasm` — the full tool (format + parse) in one package, with a
  `tsv` bin (`format` + `parse` subcommands mirroring the native CLI's flags and
  exit codes; single-threaded WASM — `--jobs` is accepted and ignored)
- slim `@fuzdev/tsv_parse_wasm` to parse-only (the `format_*` exports and their
  printers move to `@fuzdev/tsv_wasm`; wasm drops from ~2.9 MB to ~1.7 MB raw,
  ~895 KB to ~515 KB gzipped)
