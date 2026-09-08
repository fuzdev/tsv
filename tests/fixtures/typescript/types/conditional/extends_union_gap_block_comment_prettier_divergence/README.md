# extends_union_gap_block_comment_prettier_divergence

A block comment glued ahead of a should-hug union's first member at a conditional's
`extends` (`T extends /* c */ { … } | null ? 1 : 2`), where the member fits on the
continuation line after `extends` but not beside it.

Both formatters bind the glued comment to the first member — the union declines its
hug and breaks after `extends` — and both print the same first pass from the glued
one-line authoring: `input.svelte`. (That authoring carries no `unformatted_*` pin:
prettier reaches `input.svelte` from it in one pass and then walks on, a chain no
variant marker expresses.) The divergence is what happens next:

- **tsv** stops there: the union's flat continuation line is its fixed point, the
  same form it prints at every sibling keyword (a type parameter's `extends` / `=`,
  a cast's `as`, the branch `?` / `:`).
- **Prettier** re-reads its own output **non-idempotently**: the block it just
  broke ahead of now sits on the line after `extends`, its conditional-type
  handler re-binds it to the union instead of the member, the union hugs and the
  object expands (`output_prettier.svelte`, one pass from `input.svelte`), and on
  the next pass the comment — glued again — binds back to the member, leaving the
  one-per-line form with an object that would have fit flat (the chain
  `audit_signature.txt` pins). The binding flips on a newline prettier itself
  emitted.

tsv keeps one binding for both spellings, so each is one fixed point; the same rule
at every other seam matches prettier
([union_hug_gap_block_comment_keyword](../../union_hug_gap_block_comment_keyword/)).

See [conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks)
and the frame's [§Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
