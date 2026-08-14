# condition_shell_blank_before_comment_prettier_divergence

An own-line block comment inside a **stripped grouping paren shell** in a condition's
`(`→test gap, with a blank line the author wrote **ahead of it**
(`if (⏎/* c1 */⏎(⏎⏎/* c2 */⏎x⏎)⏎)`).

tsv: keeps the blank — it separated the comment from what was above it in the authored
form, and says the same thing in the erased one
Prettier: erases it with the shell

## Reason

The blank is **authorship, not shell structure**, and tsv preserves it for the same
reason it does at a binding default's shell
([stripped_paren_interior_blank_before_comment](../../../expressions/destructuring/stripped_paren_interior_blank_before_comment_prettier_divergence/)):
the parens are the author's punctuation, the comment and the blank are their prose, and
erasing the first is no reason to erase the second.

The two formatters read the blank from **opposite ends of the gap**, which is the whole
of the difference. Prettier's `printLeadingComment` measures it *forward* from the
previous comment: `skipNewline(skipSpaces(text, locEnd(comment)))` then `hasNewline` —
so the blank has to be the line immediately **after** `/* c1 */`, and the shell's `(`
sitting there ends the question. tsv asks whether the gap holds a blank line at all, and
the shell's erasure then leaves that blank directly above `/* c2 */`, where both
formatters agree it belongs.

Prettier is no oracle for this gap in either direction — it is the same reading applied
to a region one of the two erases — and it answers the mirror authorings the way tsv
does. Neither of these is a divergence, and both are pinned in
[condition_comment](../condition_comment/)'s `unformatted_stripped_paren.svelte`:

| authoring | prettier | tsv |
| --- | --- | --- |
| blank **above** the shell, `/* c1 */⏎⏎(⏎/* c2 */` | keeps | keeps |
| blank in the shell's **tail**, `(⏎x⏎⏎)⏎/* c */` | erases | erases |
| **this fixture** — blank in the shell's head, `/* c1 */⏎(⏎⏎/* c2 */` | erases | **keeps** |

The trailing row is prettier's own `isPreviousLineEmpty`, asked of the comment, which is
the reading tsv uses at that gap too — a condition→`)` comment is a *trailing* comment of
the test in both formatters, where a `(`→test comment is a *leading* one. So the rule
that parts here is prettier's, per run, and tsv follows it wherever the run is trailing.

## Cases

`if` and `while` (the shared condition builder) and `do…while`, whose `(`→condition run
is emitted by its own arm — the one that preserves an inline comment after `(` for the
do-while relocation divergence, so it is a second spelling of this gap rather than a
second construct through the first.

- `unformatted_ours_stripped_paren.svelte` — the authored shell form: tsv normalizes it
  to input, prettier to the variant.
- `variant_stripped_paren.svelte` — prettier's landing form, dual-stable: with no blank
  left to keep, tsv keeps that form as-is, so an already-prettier-formatted file does not
  churn.

See [conformance_prettier_ts_comments.md](../../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation, and
[comments.md](../../../../../../docs/comments.md) for the two readings and which run each
belongs to.
