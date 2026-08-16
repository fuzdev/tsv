# branch_paren_trailing_line_comment_prettier_divergence

A redundant paren shell around a **nested conditional branch** whose trailing gap holds a
**line** comment. The branch prints the clarity pair it decides on rather than the one the
author typed, so the shell is ordinarily stripped and the comment deferred — which is
lossless only while the enclosing conditional still has a `:` to come.

**tsv**: in the **false** position nothing but the statement tail follows, so the shell is
**retained** and opened, the comment keeping the line it was written on:

```
type A = T extends U
	? Z
	: (
		V extends W ? X : Y // c
	);
```

**Prettier**: strips the shell in every position and carries the comment out past the `;`
(`: V extends W⏎↹? X⏎↹: Y; // c`), re-binding it from the branch to the whole statement.

That relocation is not lossless — it lands the `//` on a line that may already hold one,
where the two render back to back and the second becomes text of the first — and it is not
even a fixed point: with the shell gone, the reparse has nothing forcing the break, so a
second pass collapses the conditional back onto one line. Prettier needs **two passes** to
settle case A (`type A = T extends U ? Z : V extends W ? X : Y; // c`), pinned by this
fixture's `audit_signature.txt`; tsv reached the identical instability from its own side
until the shell was retained. Retention is what every other bracketed type region already
does with its own trailing gap. See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).

## Cases

- **A** — the divergence: false position, shell retained.
- **B** — the **true** position, where the arm's own `:` ends the output line right after
  the branch: the deferred run flushes on the branch it was written on, so the strip is
  lossless and **prettier agrees**. This is the bound on the retention, not an exception to
  it — the same carve-out `Printer::type_member_separator_follows` states for union
  members, read one construct over.
- **C** — a false-position shell nested inside an outer **true** branch. The outer `:` is
  still to come, so this strips too — which is why the question is asked of the source
  (`Printer::conditional_branch_colon_follows`, crossing `)` closers) rather than of a
  true/false flag.
- **D** — the comment-free control: the shell strips at every position.
