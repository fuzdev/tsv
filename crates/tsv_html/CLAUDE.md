# tsv_html

> HTML element classification, whitespace rules, and character entity decoding — pure functions, no AST.

Language-level utilities for HTML. Not a parser — operates on tag-name
`&str` slices. Designed to be reusable across future tools (linter,
LSP, compiler), not just the formatter. See the root
[CLAUDE.md §Language-Level concerns (classification)](../../CLAUDE.md#language-level-concerns-classification).

## Architecture Position

Zero dependencies on other `tsv_*` crates (only `phf` at runtime;
`serde_json` at build time — see `Cargo.toml`).
Current consumers: `tsv_svelte`'s printer, `tsv_svelte_compile`, and `tsv_debug`'s render/authoring audits.

The printer-adapter layer — methods that resolve span-identity names and
call into this crate — lives in `tsv_svelte/src/printer/classification/`,
not here. This crate stays AST-agnostic.

## Public API

- **Element classification** (`elements.rs`): `is_block_element`,
  `is_void_element`, `is_svg_element`, `is_mathml_element`,
  `is_foreign_element`, `is_line_break_element` (`<br>` alone — the
  element that IS a rendered line break; deliberately narrower than
  void, and `<wbr>` — a break *opportunity* — is not a member).
  Inline-ness is derived by negation in the
  consumer (matches prettier-plugin-svelte: `isInline = !isBlock`); no
  positive list is exported.
- **Custom-element name chars** (`elements.rs`): `is_pcen_char` — the one
  `char`-level predicate (the rest of the API is `&str`), a `PCENChar` per
  the HTML "valid custom element name" grammar. Shared by `tsv_svelte`'s
  tokenizer (keep a whole custom-element name in one token) and its name
  validator (the hyphen-tail run) — one source of truth for the ranges.
- **Whitespace** (`whitespace.rs`): `preserves_whitespace` (`<pre>`,
  `<textarea>`).
- **Compiler whitespace removal** (`elements.rs`): `collapses_child_whitespace`
  — whether inter-sibling whitespace inside an element is removed **entirely**
  by Svelte's compiler (`clean_nodes` `can_remove_entirely`) rather than
  collapsed to a rendered space; the exact Svelte set, a deliberate subset of
  what HTML collapses. A different question from `preserves_whitespace`.
- **Optional end tags** (`elements.rs`): `closing_tag_omitted(current, next)`
  — whether `current`'s end tag is implicitly omitted (auto-closed) when
  `next` follows; mirrors Svelte's `closing_tag_omitted` over the
  optional-end-tag table (`<li>`, `<p>`, table parts, …).
- **Entity decoding** (`entities.rs`): `decode_character_references` —
  named, decimal, and hex (lower- and uppercase) character references
  with HTML5 attribute-context rules and Windows-1252 / surrogate
  normalization. Svelte's decoder is the AST-parity target, so its
  deliberate answers are replicated quirk for quirk while its
  implementation slips are corrected to the spec — the module docs draw
  the line case by case.

## Distinctives

- **Compile-time entity table**: `build.rs` reads `src/entities.json`
  (the WHATWG HTML
  [named character references list](https://html.spec.whatwg.org/entities.json),
  name → the characters it stands for) and emits a `phf::Map` at
  `$OUT_DIR/entities_map.rs`, `include!`d by `entities.rs`. ~2,231
  entries, zero runtime init cost. The value is a `&'static str` because
  93 names stand for two code points — Svelte's own table keeps only the
  first, which is one of the decoder's cataloged corrections.
- **Pure `&str` API**: classification predicates take tag names, not
  AST nodes or a parser's name representation. Keeps this crate
  independent of any particular parser's representation.
