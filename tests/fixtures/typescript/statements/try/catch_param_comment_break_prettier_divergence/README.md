# catch_param_comment_break_prettier_divergence

A `catch` parameter's parens open on **width** in tsv, on **comment presence** in
prettier. Both formatters hold the glued form (`catch (/* c1 */ e) {`) and the
broken form stable, so `variant_broken.svelte` is dual-stable; the two part on
where they send a third authoring, the comment the author broke after
(`unformatted_ours_broke_after.svelte`, `catch (/* c1 */⏎e) {`). tsv collapses it
to the glued form; prettier expands it to the broken one.

## Reason

Prettier's catch parens are the one paren-headed head it prints **ungrouped**.
`printCatchClause` emits `["(", indent([softline, param]), softline, ") "]` when
`parameterHasComments` and `["(", param, ") "]` — with no break point at all —
when it does not, so those softlines sit in the enclosing statement's already-broken
group and open unconditionally. The predicate counts a line comment, a leading block
with a newline after it, or a trailing block with a newline before it.

tsv prints all five paren-headed constructs — `if`, `while`, do-while, `switch`,
`catch` — through the one width-driven condition group, so a comment is never itself
a reason to break: a `//` still forces the parens open (it would otherwise swallow
the `)`), and everything else is decided by the limit, like any other line tsv can
break. Uniformity across the five is worth more than matching a rule prettier applies
to one of them.

The trailing-comment cases here are matches, not divergences — they pin the two arms
of `parameterHasComments` where prettier's answer and the width's coincide.

See [conformance_prettier.md §Print Width Philosophy](../../../../../../docs/conformance_prettier.md#print-width-philosophy)
for the principle and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks)
for the catalog entry. The multiline-block face of the same rule is
[catch_param_multiline_block](../catch_param_multiline_block_prettier_divergence/).
