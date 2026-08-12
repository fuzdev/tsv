# jsdoc_type_cast_binding_default_own_line_prettier_divergence

A JSDoc cast in a **binding default** whose comment the author gave a line of its own — a
newline on both sides of it. That shape is the one the cast does not reflow: `//`-like, it
prints a **hardline** between the comment and its `(`, so the value hangs and the `=` ends
its line.

**tsv** hangs it, the same layout an own-line cast gets at every other value position (the
declarator's is pinned as the sibling's `variant_hung`). The comment and its `(` sit indented
under the `=`, and the cast survives intact.

**Prettier relocates the comment across the `=`**, out of the value gap entirely, to lead
the *binding* (`/** @type {A} */⏎objDefault = (…)`) — and that move is not idempotent:
having separated the comment from the `(` it no longer reads as a cast, so prettier's own
second pass **strips the parens** and the type assertion stops existing.
`audit_signature.txt` pins that whole chain, pass 2 included. The comment is now attached to
the binding name, which is not what `/** @type {A} */` in front of a parenthesized value
means.

That is why this authoring is a fixture rather than a variant marker: every variant marker
asserts something about a **prettier-stable** form, and prettier has none here.
`input.svelte` is tsv's own fixed point, `output_prettier.svelte` is prettier's first pass,
and the signature carries the rest.

## The layout is structural, not a preference

The hang is what keeps the authoring convergent at all, which is what makes it different
from the *width*-decided break next door
([`jsdoc_type_cast_binding_default_break`](../jsdoc_type_cast_binding_default_break_prettier_divergence/),
the mid-line authoring tsv reflows). `jsdoc_cast_comment_is_own_line` drives **both** halves —
the cast's hardline and the enclosing layout's hang — and they have to agree: a hardline with
no hang leaves the `(` at the binding's own indent, a form the next pass collapses back to
the reflowed one. The three binding defaults reach their layout through
`build_assignment_pattern_doc` rather than the shared assignment layout, so each has to
apply the rule itself (`Printer::is_own_line_jsdoc_cast`).

⚠️ That predicate is deliberately **narrower** than `owned_leading_comment_effect`, which
also hangs an *indentable* owned block (`= /*⏎ * c⏎ */ 1`). Prettier hangs such a block at a
**declarator** and keeps it inline at a binding default; tsv matches both, so widening the
test here would break a match that already holds
([member_init_multiline_block_comment](../../../declarations/enum/member_init_multiline_block_comment/)
pins the enum spelling of it). Only the cast's own hardline makes the hang structural.

The same rule at the fourth site this gap reaches — an enum member's `=`, TypeScript-only —
rides in [`jsdoc_type_cast_enum_member_break`](../jsdoc_type_cast_enum_member_break_prettier_divergence/)
as its `C` member.

## Reason

**Comment position.** Prettier moves the comment across the `=` to a different syntactic
position, and in doing so loses the cast — the standing
[§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
difference in its sharpest form, since the relocation is information-losing rather than merely
cosmetic. tsv keeps the comment in the gap the author wrote it in.

See [conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and [§JSDoc / paren semantics](../../../../../../docs/conformance_prettier_ts_comments.md#jsdoc--paren-semantics).
