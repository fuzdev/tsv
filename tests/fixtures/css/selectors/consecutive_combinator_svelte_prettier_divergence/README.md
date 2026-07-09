# consecutive_combinator_svelte_prettier_divergence

A **run of consecutive combinators** — two or more with no compound selector between
them (`> > .a`, `.b > > .c`, `+ ~ .d`, glued `>>.a`). This is invalid CSS (a
combinator must join two compounds; a browser discards the whole rule), but both
canonical tools parse it permissively, and they disagree with tsv on what to keep.

## Svelte divergence (AST)

Svelte's `parseCss` **collapses** the run to its last combinator — its `read_selector`
never emits an empty relative selector, so on reaching a second combinator with no
simple selectors accumulated it *drops* the earlier one (`+ ~ .d` → a single
`RelativeSelector` for `~ .d`). tsv instead **preserves every authored combinator**,
emitting an empty-compound `RelativeSelector` for each anchorless one (`+ ~ .d` →
`[RelativeSelector(+, [])]` then `[RelativeSelector(~, [.d])]`). So `expected_ours.json`
carries the extra empty relative selectors that `expected_svelte.json` drops.

tsv declines the collapse because it is a **lossy recovery**: the dropped combinator is
authorship, and destroying it at parse time blinds the future diagnostics layer to the
error — in a relative context the collapse even makes the invalid selector silently
*valid* (`:has(+ ~ .d)` → `:has(~ .d)`). Preserving keeps the mistake visible for
diagnostics to flag, the same permissive-parser posture ("accept, defer validity, do not
silently recover") tsv takes elsewhere. Only the leading null-anchor (a first relative
selector with neither combinator nor compound) is dropped, matching parseCss.

## Prettier divergence (formatting)

tsv keeps the run, normalizing only the whitespace to its single-space convention
(glued `>>.a` → `> > .a`). Prettier **collapses** the whitespace-separated run to its
last combinator (`> > .a` → `> .a`), so `output_prettier.svelte` is the collapsed form.
A *glued* run prettier freezes verbatim instead (`>>.a` stays `>>.a`);
`prettier_variant_glued` pins those prettier-stable forms, which tsv normalizes to
`input.svelte`. `input_invalid_trailing_combinator.svelte` covers a *trailing* combinator
(`.a > > {}` — a run with no final compound), which both parsers reject.

See [conformance_svelte.md §CSS Corrections](../../../../../docs/conformance_svelte.md#css-corrections)
and [conformance_prettier.md §CSS: Selectors](../../../../../docs/conformance_prettier.md#css-selectors).
