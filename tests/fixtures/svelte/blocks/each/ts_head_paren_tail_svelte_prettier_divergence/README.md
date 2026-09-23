# ts_head_paren_tail_svelte_prettier_divergence

Under `lang="ts"`, an `{#each}` head whose expression ends on a parenthesized operand —
`a || (b as A)`, `cond ? a : (b as A)` — ends at that `)` in tsv. Canonical Svelte ends it
one character early, before the `)`, and prettier-plugin-svelte, which prints the head from
that offset, drops the paren.

## Svelte divergence (parser)

acorn-typescript reads the head past its separator — `(b as A) as item` is one
`TSAsExpression` — and Svelte's `{#each}` reader unwinds it (`1-parse/state/tag.js`): the
`TSAsExpression` that ends the expression is replaced by its own expression, and the head's
`end` is set to *that* node's `end`. acorn keeps no grouping parens, so the unwrapped
`b as A` ends before its `)`, and the head expression — a `LogicalExpression`, then a
`ConditionalExpression` — gets an `end` inside its own source. Its `loc.end` is not touched
at all and stays past the binding, the separate each-`as` stale `loc.end` correction. tsv
reads the separator directly (`tsv_ts::TopLevelAs`) and never unwinds, so the head ends at
its last token, the `)`. See
[conformance_svelte.md §Svelte Template Corrections](../../../../../../docs/conformance_svelte.md#svelte-template-corrections-corpus-enforced).

## Prettier divergence (formatter)

◆prettier_bug. prettier-plugin-svelte prints the head expression from the source slice
Svelte's offsets bound, `a || (b as A`. The slice does not parse, the plugin falls back to
printing it verbatim, and then writes ` as item` — so the output is
`{#each a || (b as A as item}`, which no parser accepts; prettier's own next pass throws on
it (F4b tolerates the missing `audit_signature.txt`). tsv keeps the pair. See
[conformance_prettier_svelte.md §Svelte: each-head parenthesized tail under `lang="ts"`](../../../../../../docs/conformance_prettier_svelte.md#svelte-each-head-parenthesized-tail-under-langts).

## Related

- [ts_head_comment_duplication](../ts_head_comment_duplication_svelte_prettier_divergence/) — the same unwind, seen from the comments its discarded parse doubles
- [type_assertion_comment](../type_assertion_comment_svelte_prettier_divergence/) — the same unwind, seen from the leading-comment attachment it drops
