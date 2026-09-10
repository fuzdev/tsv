# else_blank_between_comments_prettier_divergence

An authored blank line **between two own-line comments** in the `}`→`else` gap.

tsv: preserves it — a blank between two comments separates two distinct remarks
Prettier: collapses it, in this gap only

## Reason

tsv preserves the blank **uniformly, in every gap that can hold two own-line comments**.
Prettier preserves it in every one of those gaps *except* this one, so the divergence is
prettier's inconsistency, not tsv's. Measured (prettier 3.9.0, same input, only the
enclosing context varying):

| context | prettier | tsv |
| --- | --- | --- |
| statement list (function body) | preserves | preserves |
| class body | preserves | preserves |
| object literal | preserves | preserves |
| array elements | preserves | preserves |
| parameter list | preserves | preserves |
| header `)`→body (`if (a)⏎// c1⏎⏎// c2⏎{`) | preserves | preserves |
| `}`→`catch` | *relocates the comments into the body — **carrying the blank with them*** | preserves in place |
| do-while `}`→`while` | *relocates into the condition parens — **carrying the blank*** | preserves in place |
| **`}`→`else` / `else if`** | **collapses** | **preserves** |
| **non-block consequent's `;`→`else`** | **collapses** | **preserves** |

The two relocating rows are the telling ones: even where prettier moves the comments out
of the gap entirely, it keeps the blank between them. It treats the blank as meaningful
everywhere except the one gap where it happens to print the comments through a different
path. An authored blank between two comments is the same authoring signal as a blank
between two statements — which both formatters preserve — so tsv keeps it here too.

## Cases

Line comments, block comments, a longer run (every blank in it), and `else if` — the
divergence is uniform across all four, and is *only* about the blank; the comments' own
positions match prettier exactly.

A **non-block** consequent reaches the same gap by a different route — through its own
terminator — and answers the same way. That route is the one place the blank is read by
the clause-tail deferral rather than by the `}`→`else` gap: the consequent's tail stays
open to the `else`, so its terminator-gap run rides in `line_suffix` docs and each member
carries its own break and its own blank. `unformatted_ours_pre_terminator.svelte` is the
authoring that exercises it, with the comments written *before* the `;` — including a run
the author glued onto one line, which breaks between its members (that split is not this
fixture's subject; see
[clause_terminator_gap_glued_comment_run](../clause_terminator_gap_glued_comment_run/))
while the authored blank between the two remarks survives it.

Not covered here (each is a different question with its own fixture): a blank **above**
the first comment in this gap is preserved by both — [else_blank_before_comment](../else_blank_before_comment/);
a blank above the first comment in the **header→body** gap is dropped by both —
§"No blank above a body block's `{`".

`variant_collapsed.svelte` (prettier's form) is dual-stable: tsv keeps it as-is, so an
already-prettier-formatted file does not churn.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md) §"No blank above a body block's `{`".
