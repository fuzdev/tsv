# operator_value_forced_break_blank_prettier_divergence

A block comment on an `=` or `:` operator's line whose break is **forced** by a line comment
below it, with an author blank between them (`const a1 = /* x */⏎⏎// y⏎b`). The blank survives
with the break, as it does at every forced continuation
([§Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position),
the converse paragraph) and as the keyword→value gaps keep it
([keyword_value_forced_break_blank](../keyword_value_forced_break_blank_prettier_divergence/)).

tsv (`input.svelte`) keeps both comments where the author wrote them, the blank between them,
and hangs the value one level in:

```ts
const a1 = /* x */

→// y
→b;
```

Prettier (`output_prettier.svelte`) moves the block comment at every site, so the divergence is
the block's placement; where the blank goes follows from the move:

- the declarator, assignment and for-loop init `=`: the block drops onto its own line below the
  `=`, and the blank is **kept** after it;
- the object property: the block is hoisted above the key, the blank kept below it;
- the enum member: the block moves in front of the `=` and the `//` trails the `=`, the blank
  dropped;
- the import attribute: the block moves in front of the `:`, the blank dropped. Prettier's form
  is not its own fixed point there, which `audit_signature.txt` pins.

The blank after a `//` on the operator's line is a different case: prettier drops it at the
declarator `=` while leaving the comment in place, and tsv drops it too
([multi_block_comment_after_eq](../../../declarations/variable/multi_block_comment_after_eq/)).

The cases cover the value gaps that share the `=`/`:` line-comment partition
(`build_operator_line_comment_hang`): the declarator `=`, an assignment expression, an enum
member, a for-loop init, an object property's `:` and an import attribute's `:`.

See
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and
[conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).
