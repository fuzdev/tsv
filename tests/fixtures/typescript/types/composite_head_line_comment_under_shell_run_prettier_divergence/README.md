# composite_head_line_comment_under_shell_run_prettier_divergence

A **sole-member composite whose head gap holds a `//`, inside a redundant paren shell that
carries a leading run of its own** — `b & (/* c */| // c⏎a)`, `b | (// c⏎& // c⏎a)` — as a
**later member** of an intersection or a union.

The shell strips and the one-member composite prints transparently as its member (prettier
drops the node in postprocess), so **both** halves of the leading region are the enclosing
member gap's: the shell's own run and the `//` the author wrote after the operator are ONE
run, landing exactly where the operator-less authoring
([intersection_redundant_paren_member_line_comment](../intersection_redundant_paren_member_line_comment_prettier_divergence/))
puts it. The operator never survives, and the member keeps the member gap's own indent — the
same claim
[intersection_member_single_member_head_line_comment](../intersection_member_single_member_head_line_comment_prettier_divergence/)
makes for the shell with no run of its own.

Three shell-run kinds reach it, because the shell's own leading-run emitter answers each
differently: a **glued block** (`(/* c1 */|`), an **own-line `//`** (`(// c3⏎|`), and a
**multi-line block** (`(/* a5⏎b5 */|`).

- `unformatted_ours_pipe.svelte` / `unformatted_ours_amp.svelte` — the pipe and ampersand
  authorings; tsv normalizes both to `input.svelte`. Prettier does not: it lifts the shell's
  block onto the operator's own line (`b /* c1 */ & // c2`) and, at the union, onto the
  previous member's (`| b /* c7 */ // c8`) — `variant_lifted.svelte`, one form both
  formatters then hold stable, so the two authorings are dual-stable rather than one fixed
  point. Prettier keeps `input.svelte` itself stable (no `output_prettier`).

## Reason

Transparency, at one more layer than the sibling fixtures ask about. The composite has no
operator to print, so a layout it chose for its own head gap was a second fixed point — or
none — for one program: the union's leading-pipe layout (`| // c⏎  a`), which after a `&` or
a `|` **does not reparse at all**, and the intersection's indent shell, read back one level
shallower. A run in the shell around the composite does not change that; it only changes
which emitter prints the run's first half, so the two must answer as one.

See [conformance_prettier.md §Comment Position Philosophy](../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in [conformance_prettier_ts_comments.md §Comment normalization](../../../../../docs/conformance_prettier_ts_comments.md#comment-normalization-stable-quirks).
