# intersection_leading_gap_shell_run_prettier_divergence

An intersection written with the **leading-operator** syntax (`& A`, whose span opens at the
`&`) whose leading gap holds comments **and** whose first member is a redundant paren shell
holding a leading `//`. The two windows are contiguous in source, so they are **one run**,
emitted in source order and exactly once.

tsv, at `A`:

```ts
type A = /* c1 */
	// d1
	a;
```

## Reason: contiguous windows are one run, and one run has one emitter

The first member's shell strips, so its `//` hoists out of the intersection
(`intersection_first_member_hoist_comments`). The gap ahead of it — everything between the
intersection's own `&` and that member — is the intersection's, and every other spelling has
the two coincide, because the span starts *at* the first member. The leading-operator
spelling is the one place they don't, and three things went wrong there at once:

- the hoist route emitted only the shell's run, and its compact body never scanned the
  leading gap at all, so a block comment there was silently **DROPPED**
  ([comments.md](../../../../../docs/comments.md) hazard 4);
- where the body *was* the general compact one, the gap printed **twice** — once from an
  enclosing seam whose leading-edge claim had widened over the shell, and once from the
  intersection itself. The claim names the shell's region while the enclosing gap's *window*
  opens at its own start, so it swallowed bytes nobody had handed it — an anchor shift, not
  a partition. The seam now declines the descent for a leading-operator intersection whose
  gap actually holds a comment (empty, the two windows coincide and the claim is exactly the
  shell's, which the array / indexed-access / conditional positions still need);
- emitted by the body *after* the hoisted run, the pair came back **REVERSED** — `// d1`
  ahead of `/* c1 */`, which the author wrote first.

`H` is the same run reached with a **line** comment in the gap, which routes through
`build_intersection_leading_gap_line_comment_doc` — the sibling that already composed the
two windows this way, and the reason the hoist route now spells it identically.

## Prettier

Prettier drops the whole run onto its own line after the `=`; tsv trails the first comment
on the `=` and indents the continuation, the placement
[intersection_leading_line_comment](../intersection_leading_line_comment_prettier_divergence/)
catalogs. Prettier's chain from the authored form is pinned by `audit_signature_authored.txt`.

## Files

`unformatted_ours_authored.svelte` is the authored form — the `&`, the gap comments and the
shell all present; tsv normalizes it to `input.svelte`.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md) §Comment relocation.
