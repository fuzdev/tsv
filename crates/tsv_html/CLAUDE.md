# tsv_html

> HTML spec data and the answers the HTML spec gives — a pure function over text, no AST.

The published home of what tsv reads from the HTML spec: the named character references
table. The long-term target is a spec-faithful HTML library, so the public surface holds only
spec data and spec answers — an item whose answer is a tool's reading of the spec, however
close, does not belong here. The answers Svelte's compiler and prettier-plugin-svelte give
(block / void / foreign classification, the whitespace sets, Svelte's auto-close table and
tag-name tail class, Svelte's character-reference decoder) are tool answers, not the spec's,
and live privately in `tsv_svelte` (`src/html/`). A spec-faithful optional-tags table or
custom-element name check would be added here as new items, beside Svelte's private ones. See
the root [CLAUDE.md §Language-Level concerns (classification)](../../CLAUDE.md#language-level-concerns-classification).

## Architecture Position

Zero dependencies: no `tsv_*` crate, no external crate, no build script (`serde_json`
is a dev-dependency, for the entity-table generator alone).
Current consumer: `tsv_svelte` — its private character-reference decoder (the
named-reference match).

## Public API

All at the crate root (`src/lib.rs`, a re-export over the private `entities` file).

**Stability**: the public signatures follow 0.x semver like every tsv crate (a breaking
change only at a minor release); what the lookup answers may change in any release,
tracking the spec and what tsv needs.

- **Named character references** (`entities.rs`): `match_named_character_reference(input) ->
  Option<NamedCharacterReference>` — the tokenizer's named-character-reference state: the
  longest name in the table that starts `input` (the text after a `&`). The match is a row of
  the spec's table in its own terms, private fields behind two `const` accessors: `name()`
  (the table's `'static` spelling, its `;` included when the match took one — byte-equal to
  the start of `input`, so its length is the bytes consumed and `ends_with(';')` says whether
  a `;` closed it) and `characters()` (the one or two characters it stands for). What a
  caller does with a match — the attribute-value rule for a legacy name, the parse errors —
  is the caller's (`tsv_svelte`'s decoder applies Svelte's).

## Distinctives

- **Committed entity table**: `src/entities_table.rs` is generated from
  `src/entities.json` (the WHATWG HTML
  [named character references list](https://html.spec.whatwg.org/entities.json),
  name → the characters it stands for) by `deno task entities:update`. The
  generator is the `entities_table_is_fresh` unit test (`src/entities_gen.rs`),
  which fails when the committed file is stale, so `deno task test` gates it — and skips
  (with an `eprintln!`) when `src/entities.json` is absent: the published crate excludes the
  JSON (`exclude` in `Cargo.toml`, graded by `package:audit`), so a test run from the tarball
  has nothing to grade the table against. The
  table is a perfect hash in `phf`'s CHD shape — a seed, a displacement pair per
  bucket, one name per slot — built with the lookup's own hash (`entity_hash` /
  `entity_slot` in `entities.rs`): one hash and one compare per lookup, hit or
  miss, which `match_named_character_reference`'s longest-prefix search needs since most of the names
  it asks for are not references. The lookup and the hash are `#[inline]`, so the whole
  match compiles into the decoder in `tsv_svelte` as it did when both lived in one crate.
  No build script, no code generator or JSON
  parser in a consumer's build, and no `phf` itself, whose map type changes shape
  under its `ptrhash` feature (a consumer could turn it on; a committed `phf::Map`
  literal would then stop compiling). The value is a `&'static str` because some
  names stand for two code points — Svelte's own table keeps only the first,
  which is one of the decoder's cataloged corrections. Every legacy (semicolon-less) name
  has a twin with its `;`, a table fact a unit test pins and the decoder's `;` handling rests
  on. The list is WHATWG's
  under CC BY 4.0 (BSD 3-Clause where incorporated into source code). The crate ships
  only the generated table, so its license is `0BSD AND BSD-3-Clause` (its own, not the
  workspace's; every npm package states the same, `NPM_SHARED_METADATA`); `NOTICE`
  carries the attribution and both licenses, and ships in the crate — and, since
  BSD 3-Clause asks a binary distribution to carry it, in every npm package and
  every GitHub Release beside the CLI binaries (`THIRD_PARTY_NOTICE` in
  `scripts/npm_metadata.ts` names it; the staging scripts copy it verbatim and
  `validate:artifacts` and the package suites fail a package without it). `tsv_svelte`
  carries none of the table (it calls `match_named_character_reference`), so its license stays the
  workspace's 0BSD.
- **Pure `&str` API**: the lookup takes text, not AST nodes or a parser's name
  representation, and answers the table's own `'static` strings, borrowing nothing from its
  input, so the crate stays independent of any particular parser.
- **Exact, case-sensitive names**: the lookup matches a name as authored, as the spec's
  table does (`&Aacute;` and `&aacute;` are different references). Stated once in the crate
  `//!` docs.
