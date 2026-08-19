# before_eq_comment_value_head_freeze_prettier_divergence

The **operator→value gap inside the before-`=` continuation**. When a comment the author
wrote before the `=` cannot be held inline (a `//` would swallow the operator), tsv keeps
it after the name and drops `= value` to a continuation line — the family's rule, already
cataloged per host at
[declarator_before_eq_line_comment](../declarator_before_eq_line_comment_prettier_divergence/),
[property_before_eq_line_comment](../../class/property_before_eq_line_comment_prettier_divergence/)
and
[before_operator_line_comment](../../../expressions/assignment/before_operator_line_comment_prettier_divergence/).

What this fixture adds is what happens **inside** that continuation: the `=`→value gap
keeps its own rule there. A comment the author gave a line of its own leads the value from
its own line, exactly as it does when nothing precedes the `=`; only a comment on the `=`'s
line stays on it. The continuation used to glue the run to the operator with an
unconditional space, which made this the one arm of the family that flattened that
distinction.

For an honored `prettier-ignore` that was worse than a layout difference. A directive
**trailing** an operator is inert under tsv's own floor, so the relocated form could not be
read back as a freeze: the next pass normalized the value and the directive's whole effect
disappeared — a silent loss no gate catches, since nothing is dropped and the wrong output
is its own fixed point. Keeping the directive's line is what makes the freeze survive the
round trip.

Three cases carry the rest of that "same rule inside as outside" claim, because each is a
way the gap's own answer used to stop at the continuation's edge:

- **`c10` — an own-line JSDoc cast.** A cast OWNS its comment, so the gap's to-emit lookup
  reports the gap empty; but the cast prints a hardline of its own between the comment and
  its `(`, so a space separator welds the annotation to the `=` and strands the `(` at the
  continuation's indent. The separator asks the cast by name. `c11` is the glued authoring,
  the control that must NOT gain a break.
- **`c13` — a block glued to the operator.** The value pulls up onto the comment's line
  across the author's newline, the same pull-up the ordinary arm makes (`const a = /* c */ v`)
  and prettier with it. The trigger is a newline this fixed point erases, so it is authored
  in `unformatted_ours_hug.svelte` rather than here; the tail is grouped so the hanging run's
  own hardline no longer decides that soft break for it.

## Why tsv differs

Prettier relocates the before-`=` comment **past the operator** at every host here, which
is the family divergence the sibling fixtures above already sanction — with a second
comment already trailing the construct, prettier merges the two onto one line and the
second `//` stops being a comment, so tsv preserves the authored position instead.

Prettier is not idempotent on its own output: the control case's second comment (`// c8`)
lands at one indent level less on the first pass and settles on the next, pinned by
`audit_signature.txt`.

## Expected behavior

- **tsv**: the before-`=` comment stays after the name; the `=` and value take a
  continuation line; the directive keeps its own line and the value prints verbatim; the
  value's clarity parens are the position's and the gap's content cannot strip them. The
  input is a fixed point.
- **prettier**: relocates the before-`=` comment past the operator (see
  `output_prettier.svelte`), honoring the freeze either way — and agreeing with tsv on
  everything the continuation's own gap decides: the cast's own line, the glued cast's, and
  the block's pull-up.
- **`unformatted_ours_hug.svelte`**: the `c13` case authored broken. It normalizes to
  `input.svelte` under tsv; prettier's chain from it converges on neither, so it is pinned
  by `audit_signature_hug.txt` (rule N12) rather than by a single-form marker.

The last two cases are the remaining hosts of the family — an **enum member** (`c14`) and a
**`for`-header init declarator** (`c15`), whose clause separator is a `;` rather than a
statement terminator. At the enum member prettier's relocation is **lossy**: it merges the
directive and the before-`=` comment onto one line (`Bbb = // prettier-ignore // c14`, where
`// c14` stops being a comment), and its own second pass then floats the merged run past the
value and normalizes it, so the freeze the author asked for disappears. That merge is the
family's whole argument, at the one host where it also costs the freeze.

## Reason

◆comment_preservation — tsv preserves the authored position wherever relocating it would
merge two comments into one. Sanctioned in
[conformance_prettier_ts_comments.md §Comment relocation](../../../../../../docs/conformance_prettier_ts_comments.md#comment-relocation)
and, for the freeze half, in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*); the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
