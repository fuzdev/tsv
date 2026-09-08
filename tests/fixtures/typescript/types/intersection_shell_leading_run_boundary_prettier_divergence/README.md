# intersection_shell_leading_run_boundary_prettier_divergence

An intersection member's paren shell holding **both** a leading `//` and a lifted trailing
run. The leading run is relocated — hoisted above the intersection for the first member,
placed ahead of the member for a later one — and the question this directory pins is where
the boundary that follows then puts the **trailing** run.

Both halves are one rule: **a relocated shell run has to leave the boundary in the state the
reparse will read it back in.** The run is a deferred `line_suffix` and renders its own break
at the indent it was QUEUED at, so a boundary that hugs past it carries the run out past the
`;` — onto a line the reparse cannot re-break.

## The first member (c1–c15)

The hoist route rebuilds the first member with its parens stripped. Its lifted trailing run
is still only placeable at the boundary that FOLLOWS it, exactly as at every other position
([union_intersection_parens_line_comment_run](../union_intersection_parens_line_comment_run/)
cases AP–AW). Only a **required** pair reaches this route — a bare inner's shell run is
claimed by the enclosing `=` gap instead, which is case `E`, the control that already gave
this layout by the other route.

## A later member (c16–c23)

Prettier's `hasLeadingOwnLineComment(originalText, node)` disjunct
(`printIntersectionType`), asked over the region tsv's AST hides: the member's **own paren
shell**. The router's window runs between the member SPANS
(`Printer::intersection_has_isolated_member_comment`) and tsv keeps the
`TSParenthesizedType` node, so a run written inside the shell falls inside the member's span
and the window is empty. Prettier's parser drops the paren, so the same comments simply lead
the member and its existing disjunct sees them. This is the leading-run sibling of
`Printer::intersection_boundary_leading_run_ends_line`, which asks the same question over the
PREVIOUS member's shell trailing gap.

It is a question about the LEADING run alone: a shell with no trailing run to lift opens its
boundary just the same (`I`), and a block-led one does not (`J`) — a block ends no line, so
nothing is relocated and prettier's own predicate would say no too.

## Both loops, and the pair that must stay closed

Every rule here is asked at the compact loop AND at the forced-multiline one (`K`, `L`) —
held at one only, an authoring reaches two fixed points depending on whether some unrelated
gap happens to carry an isolated comment, which is this module's standing twin-loop hazard.
The hoisted run's own indent is the alias `=` gap's at both: the multiline layout indents
CONTINUATION members, while the first member sits at the body's base, which is the line the
run drops onto.

`M` and `N` pin the other half of "the hoist is a claim": the pair the member position
REQUIRES must stay CLOSED, because the run above is already its emitter
(`Printer::required_paren_open_run`'s shell arm takes the same claim filter its edge arm
has). With no trailing run to lift there is no hoisted member and no boundary question — so
nothing else in this directory would have caught the double print.

## The divergences

Cases `F` and `G` are byte-for-byte **prettier**. What diverges:

- **c1–c15** — where the hoisted run goes. tsv trails the `=` and hangs the value; prettier
  breaks after the `=` and drops the run onto its own line. The pre-existing sanction of
  [intersection_redundant_paren_first_member_trailing_line_comment](../intersection_redundant_paren_first_member_trailing_line_comment_prettier_divergence/),
  reached here because the hoist lands the run in the alias's `=` gap.
- **c21–c23** — prettier attaches the shell's leading `//` as a TRAILING comment of the
  member above it (the comment follows content on its line — the `(`), so its own
  `hasLeadingOwnLineComment` is false and its first arm hugs the both-objects boundary before
  the question is asked. It then `lineSuffix`-relocates the comment to the end of the visual
  line. tsv keeps the run leading the member the author wrote it inside.

⚠️ Prettier is **non-idempotent** on much of this family: `audit_signature.txt` pins its chain
from `output_prettier.svelte`, and `audit_signature_shell.txt` the chain from the paren
authoring. Grading tsv against prettier's first pass here says nothing.

## Files

`unformatted_ours_shell.svelte` carries the paren authoring, which reaches `input` under tsv
only.

See
[conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy)
and
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation).
