# intersection_member_operator_gap_comment_prettier_divergence

A **block** comment in an intersection's member→member gap that shares a line
with something — the `&` itself, the previous member, or another comment. Such a
comment does not own its line, so it forces nothing: the intersection stays
inline and the comment keeps the side of the `&` the author chose.

- `unformatted_ours_after_operator.svelte` — the operator pushed onto its own
  line with the comment behind it (`X⏎& /* a */⏎Y`). tsv normalizes it to input,
  the comment still after the `&`; prettier re-binds the comment to the previous
  member and lands on `variant_comment_before_operator`.
- `unformatted_ours_before_operator.svelte` — the mirror authoring, the comment
  starting a line with the `&` glued behind it (`X⏎/* b */ &⏎Y`). tsv keeps it
  before the `&`; prettier re-binds it to the following member and lands on
  `variant_comment_after_operator`.
- `unformatted_glued_run.svelte` — a glued pair the author gave a line of its own
  (`X &⏎/* c1 */ /* c2 */⏎Y`). Neither comment is isolated (each is adjacent to
  the other), so **both** formatters collapse the run inline: a plain
  `unformatted_*`.
- `variant_comment_before_operator.svelte` / `variant_comment_after_operator.svelte`
  — prettier's two landing forms, each dual-stable. That they are stable in both
  formatters is the point: the side of the `&` is authorship, and tsv preserves
  whichever the author wrote instead of canonicalizing to one.

An **isolated** block — a line of its own on both sides (`X &⏎/* c */⏎Y`) — still
forces the intersection one member per line in both formatters, as does any line
comment; those are the plain matches in
[intersection_member_own_line_comment](../intersection_member_own_line_comment/).

## Reason

Per [conformance_prettier.md](../../../../../docs/conformance_prettier.md)
§Comment Position Philosophy, a comment between an operator and its operand stays
there, and own-line-ness is read from the **source** — a comment is on its own
line only when a newline both precedes and follows it, which is exactly the
condition prettier's own `printLeadingComment` uses to emit a hardline. An
intersection flattens when it fits, so the author's break *around* the `&` is
layout, not own-line-ness.

See [conformance_prettier_ts_comments.md](../../../../../docs/conformance_prettier_ts_comments.md)
§Comment relocation.
