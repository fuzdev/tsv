# Comments on BOTH sides of a detached `;`

An own-line comment in a statement's terminator gap **and** a comment trailing that
statement's own `;` on the terminator's line. The two sides take different seams —
the gap's comment is the LIST's and leads the next statement
([comment_before_detached_semicolon](../comment_before_detached_semicolon/)); the `;`-line
comment trails the statement, because the `;` prints back up on the content's line
([comment_after_detached_semicolon](../comment_after_detached_semicolon/)) — and here they
would cross.

tsv's trailing claim is a **prefix**: the first comment handed forward hands every later
one forward with it, so an own-line comment in the gap stops the run and the `;`-line
comment goes with it. Everything stays in authored order, one comment per line. Prettier
splits the two instead, printing the `;`-line comment as the statement's trailer and the
earlier gap comment below it — a **reorder** (`fn1(); /* c2 */⏎/* c1 */`), pinned
dual-stable as `variant_reordered.svelte`; both formatters keep either landing once
written.

The two controls are where the claim is not split and both formatters agree: a gap
comment on the CONTENT's own line joins the trailing run in order (`fn7(); /* c8 */
/* c9 */`), and a `//` there closes the output line, so the `;`-line comment takes one of
its own.

Reason: authored order over the reorder — the same stance as the decorator and
`;`→`else` trailer entries. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
