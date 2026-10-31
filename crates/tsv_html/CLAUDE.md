# tsv_html

> HTML element classification, whitespace rules, and character entity decoding — pure functions, no AST.

Language-level HTML utilities over tag-name `&str` slices — not a parser.
Reusable across future tools (linter, LSP, compiler), not just the
formatter. See the root
[CLAUDE.md §Language-Level concerns (classification)](../../CLAUDE.md#language-level-concerns-classification).

## Architecture Position

Zero dependencies: no `tsv_*` crate, no external crate, no build script (`serde_json`
is a dev-dependency, for the entity-table generator alone).
Current consumers: `tsv_svelte` (its tokenizer, its parser — the element's `TagFacts` and
implicit tag closing — and its printer), `tsv_svelte_compile`, and `tsv_debug`'s whitespace-aware audits (render normalization, authoring, blank, razor).

The printer-adapter layer — methods that resolve span-identity names and
call into this crate — lives in `tsv_svelte/src/printer/classification/`,
not here. This crate stays AST-agnostic.

## Public API

- **Element classification** (`elements.rs`): `is_block_element` (the elements that render
  a box of their own — UA `display: block`, plus `<li>` and `<table>` — as prettier-plugin-svelte
  lists them — a subset: `<center>`, `<legend>`, `<search>`, `<listing>`, `<plaintext>` and
  `<xmp>` render as blocks but format inline, as in the plugin; not HTML's "flow content",
  which holds the inline elements too),
  `is_void_element` (elements only — Svelte's `is_void` also answers for `!doctype`, which
  `tsv_svelte::ast::is_svelte_void` adds for the callers reading Svelte's answer), `is_svg_element`, `is_mathml_element`,
  `is_foreign_element`, `is_line_break_element` (`<br>` alone — the
  element that IS a rendered line break; deliberately narrower than
  void, and `<wbr>` — a break *opportunity* — is not a member).
  Inline-ness is derived by negation in the
  consumer (matches prettier-plugin-svelte: `isInline = !isBlock`); no
  positive list is exported.
- **Custom-element name chars** (`elements.rs`): `is_custom_element_name_char` — the one
  `char`-level predicate (the rest of the API is `&str`): the tail class of Svelte's
  `REGEX_VALID_TAG_NAME` — HTML's `PCENChar` ranges plus ASCII uppercase. Shared by `tsv_svelte`'s
  tokenizer (keep a whole custom-element name in one token) and its name
  validator (the hyphen-tail run) — one source of truth for the ranges.
- **Whitespace** (`whitespace.rs`): `preserves_whitespace` (`<pre>`,
  `<textarea>`) — Svelte's and prettier-plugin-svelte's set, not the HTML spec's
  `white-space: pre` rule (which also names the obsolete `listing`, `plaintext`, `xmp`).
- **Compiler whitespace removal** (`elements.rs`): `collapses_child_whitespace`
  — whether inter-sibling whitespace inside an element is removed **entirely**
  by Svelte's compiler (`clean_nodes` `can_remove_entirely`) rather than
  collapsed to a rendered space; the exact Svelte set, a deliberate subset of
  what HTML collapses. A different question from `preserves_whitespace`.
- **Optional end tags** (`elements.rs`): `optional_end_tag(current)` →
  `Option<OptionalEndTag>`, then `OptionalEndTag::is_closed_by(next)` —
  Svelte's `closing_tag_omitted(current, next)` over the optional-end-tag
  table (`<li>`, `<p>`, table parts, …), split so a parser settles whether an
  element has a row once and asks the trigger per child. Having a row at all
  is the `next` = end-of-parent answer.
- **Entity decoding** (`entities.rs`): `decode_character_references(html, ReferenceContext)` —
  named, decimal, and hex (lower- and uppercase) character references
  with the HTML attribute-context rule and Windows-1252 / surrogate
  normalization. Svelte's decoder is the AST-parity target, so its
  deliberate answers are replicated quirk for quirk while its
  implementation slips are corrected to the spec — the module docs draw
  the line case by case.

## Distinctives

- **Committed entity table**: `src/entities_table.rs` is generated from
  `src/entities.json` (the WHATWG HTML
  [named character references list](https://html.spec.whatwg.org/entities.json),
  name → the characters it stands for) by `deno task entities:update`. The
  generator is the `entities_table_is_fresh` unit test (`src/entities_gen.rs`),
  which fails when the committed file is stale, so `deno task test` gates it. The
  table is a perfect hash in `phf`'s CHD shape — a seed, a displacement pair per
  bucket, one name per slot — built with the decoder's own hash (`entity_hash` /
  `entity_slot` in `entities.rs`): one hash and one compare per lookup, hit or
  miss, which the decoder's longest-prefix search needs since most of the names
  it asks for are not references. No build script, no code generator or JSON
  parser in a consumer's build, and no `phf` itself, whose map type changes shape
  under its `ptrhash` feature (a consumer could turn it on; a committed `phf::Map`
  literal would then stop compiling). The value is a `&'static str` because some
  names stand for two code points — Svelte's own table keeps only the first,
  which is one of the decoder's cataloged corrections. The list is WHATWG's
  under CC BY 4.0 (BSD 3-Clause where incorporated into source code); `NOTICE`
  carries the attribution and both licenses, and ships in the crate — and, since
  BSD 3-Clause asks a binary distribution to carry it, in every npm package and
  every GitHub Release beside the CLI binaries (`THIRD_PARTY_NOTICE` in
  `scripts/npm_metadata.ts` names it; the staging scripts copy it verbatim and
  `validate:artifacts` and the package suites fail a package without it).
- **Pure `&str` API**: classification predicates take tag names, not
  AST nodes or a parser's name representation, so the crate stays
  independent of any particular parser.
- **Exact, case-sensitive names**: every predicate matches a name in its canonical spelling
  (lowercase for HTML/MathML, SVG's camelCase where it has one), as authored; a caller with a
  case-insensitive HTML name lowercases it first. Stated once in the crate `//!` docs.
- **Naming convention** (crate `//!` docs): verb predicates say what an element does
  (`preserves_whitespace`, `collapses_child_whitespace`); `is_*` names category membership.
  Docs read "Whether …".
