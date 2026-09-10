# Switch consequent tail: the last statement's terminator-gap comment

An own-line comment between a case's LAST statement's content and that statement's own
`;`. The terminator gap belongs to the enclosing list rather than to the statement
(prettier's `__contentEnd`), and for a consequent's last statement the enclosing list's
next seam is the case boundary — so the comment lands own-line at the CASE's level,
where a comment written after the consequent settles.

Both formatters agree on that landing and keep it. The divergence is the normalization
PATH: prettier's first pass leaves the comment at the CONSEQUENT's level
(`prettier_intermediate_detached_semi.svelte`), because it attaches it to the statement it
trails; only its second pass re-reads that comment as the next case's leading run and
dedents it. tsv hands the gap to the consequent list directly and reaches the settled
form in one pass.

- **tsv**: one pass from the pre-`;` authoring (`unformatted_ours_detached_semi.svelte`).
- **prettier**: two, via the consequent-level intermediate.

Uniform over a sibling case and the last case, a `//` and a trailing block in the run,
and past a dropped `;`. The controls pin the boundary: with a statement FOLLOWING in the
same consequent the comment leads it at the consequent's own level, and a block body's
last statement already lands its gap comment in place — neither has a case boundary to
hand the gap to.

Reason: one-pass convergence on a landing both formatters share. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment normalization (stable quirks)](../../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
