# scope_prelude_gap_comment_prettier_divergence

Comments in an `@scope` prelude's structural gaps — **outside** the selector
parens — are preserved at every position: leading the prelude
(`@scope /* c */ (.a)`), between the root clause and `to`
(`@scope (.a) /* c */ to (.b)`), between `to` and the limit clause
(`@scope to /* c */ (.b)`), and after the last clause before the block `{`
(`@scope (.a) /* c */ {`). Each combination of the two optional clauses is
covered (both clauses, root-only, limit-only, bare).

This is the out-of-paren counterpart of
[scope_selector_comment](../scope_selector_comment_prettier_divergence/), which
covers comments *inside* the selector parens.

## Prettier divergence

Gap spacing normalizes to a single space on each side of the comment, the same
rule as every other selector-comment position, while prettier freezes the
source spacing. `prettier_variant_compact` pins the glued forms and
`prettier_variant_spaces` the padded forms that prettier keeps stable; tsv
normalizes both to `input.svelte`. (Prettier does insert a single space after
the `@scope` name and collapses the pre-`{` run — the two spots it normalizes —
but keeps the inter-gap spacing verbatim otherwise.)

## Reason

Stable quirk. tsv registers these gap comments at parse time and re-emits them
through `comments_in_range` at each authored position, normalizing the
surrounding whitespace to a single space — the identical rule the in-paren
selector-comment path uses. Prettier preserves the source whitespace instead.
parseCss accepts the input and strips the comment from the wire `prelude`
string (leaving the whitespace, so `(.a) /* c */ to (.b)` → `(.a)  to (.b)`),
so this is a prettier-only divergence. See
[conformance_prettier.md §CSS: Comments](../../../../../docs/conformance_prettier.md#css-comments).

## Related

- [scope_selector_comment](../scope_selector_comment_prettier_divergence/) — the in-paren counterpart (comments inside the root/limit selector list)
- [scope_selector](../scope_selector_prettier_divergence/) — `@scope` selector-list whitespace normalization (no comments)
- [scope_to_case](../scope_to_case_prettier_divergence/) — `@scope`'s `to` keyword lowercased (a related delimiter-keyword canonicalization)
