# clause_terminator_comment_then_blank_prettier_divergence

A clause tail's hoisted comment with an author blank after the `;`: the comment was
written inside the statement (its terminator gap), the blank after it. Hoisting past
the `;` is the standing clause-tail landing; the blank is not crossed.

**tsv**: the comment stays with its statement, the blank follows — the authored
relative order:

```
for (;;) continue;
// c1

fn1();
```

**prettier**: `input` is a fixed point for both formatters — the divergence is the
normalization path. From the pre-`;` authoring
(`unformatted_ours_pre_terminator.svelte`), prettier carries the comment past the
blank too, re-attaching it as a leading comment of the NEXT statement
(`for (;;) continue;⏎⏎// c1⏎fn1();`) — a landing both formatters then keep
(`variant_pre_terminator.svelte`) — where tsv lands on `input` in one pass. The
block spelling behaves identically.

Reason: the `;` between the comment and the blank is structure, so the hoist is
lossless; the blank is authored separation and crossing it re-orders the pair and
re-binds the comment to a statement the author did not attach it to. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
