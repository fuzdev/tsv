# tsv_html

> HTML element classification, whitespace rules, and character entity decoding — pure functions, no AST.

Language-level utilities for HTML. Not a parser — operates on tag-name
`&str` slices. Designed to be reusable across future tools (linter,
LSP, compiler), not just the formatter. See the root
[CLAUDE.md §Language-Level concerns (classification)](../../CLAUDE.md#language-level-concerns-classification).

## Architecture Position

Zero dependencies on other `tsv_*` crates (only `phf` at runtime;
`serde_json` at build time — see `Cargo.toml`).
Current consumer: `tsv_svelte`'s printer.

The printer-adapter layer — methods that resolve span-identity names and
call into this crate — lives in `tsv_svelte/src/printer/classification/`,
not here. This crate stays AST-agnostic.

## Public API

- **Element classification** (`elements.rs`): `is_block_element`,
  `is_void_element`, `is_svg_element`, `is_mathml_element`,
  `is_foreign_element`. Inline-ness is derived by negation in the
  consumer (matches prettier-plugin-svelte: `isInline = !isBlock`); no
  positive list is exported.
- **Custom-element name chars** (`elements.rs`): `is_pcen_char` — the one
  `char`-level predicate (the rest of the API is `&str`), a `PCENChar` per
  the HTML "valid custom element name" grammar. Shared by `tsv_svelte`'s
  tokenizer (keep a whole custom-element name in one token) and its name
  validator (the hyphen-tail run) — one source of truth for the ranges.
- **Whitespace** (`whitespace.rs`): `preserves_whitespace` (`<pre>`,
  `<textarea>`).
- **Entity decoding** (`entities.rs`): `decode_character_references` —
  named, decimal, and hex (lower- and uppercase) character references
  with HTML5 attribute-context rules and Windows-1252 / surrogate
  normalization.

## Distinctives

- **Compile-time entity table**: `build.rs` reads `src/entities.json`
  (a simplified first-codepoint-only view of the WHATWG HTML
  [named character references list](https://html.spec.whatwg.org/entities.json),
  matching Svelte's runtime decoder) and emits a `phf::Map` at
  `$OUT_DIR/entities_map.rs`, `include!`d by `entities.rs`. ~2,231
  entries, zero runtime init cost.
- **Pure `&str` API**: classification predicates take tag names, not
  AST nodes or a parser's name representation. Keeps this crate
  independent of any particular parser's representation.
