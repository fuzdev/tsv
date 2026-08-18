# nonblock_consequent_gap_block_comment_prettier_divergence

A block comment in a non-block consequent's `;`→`else` gap, meeting the consequent's
own deferred pre-`;` comments at the flush before `else` — the anchor-line comment
rides the same `line_suffix` machinery as the tail run, each member with its break
inside (the gap's one-comment-per-line policy), so everything flushes in **source
order**, one line each (`docs/comments.md` §Trailing and dangling runs).

**tsv**: keeps the consequent collapsed and lands each comment in authored order, the
gap comments on their own lines before `else` — including behind an own-line block
tail and for two gap blocks:

```
if (a) expr1;
// c1
/* c2 */
else fn1();
```

A lone gap block with no deferred tail stays on the `;` line (the no-divergence
control).

**prettier on `input`**: preserves every comment placement; its one delta is the
same-line-trailer case, where it expands the consequent
(`if (b)⏎\texpr2; // c3` — `output_prettier.svelte`), the standing clause-tail
relocation of the sibling
[nonblock_consequent_trailing_comment_run](../nonblock_consequent_trailing_comment_run_prettier_divergence/).

**prettier from the pre-`;` authoring** (`unformatted_ours_pre_terminator.svelte`):
its own normalization additionally cuddles a gap block onto the `else` line
(`/* c2 */ else` — prettier's stable leading-`else` landing, which tsv normalizes
back to the own-line form; see
[else_leading_block_comment](../else_leading_block_comment_prettier_divergence/)),
through a stray-leading-space intermediate — the chain pinned in
`audit_signature_pre_terminator.txt`. tsv reaches its form in one pass.

Reason: comment order and the collapsed consequent preserved; the own-line landing
before `else` is tsv's standing leading-`else` placement. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
