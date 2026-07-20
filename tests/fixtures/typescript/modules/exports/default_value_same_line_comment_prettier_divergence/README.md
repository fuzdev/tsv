# default_value_same_line_comment_prettier_divergence

A **same-line** block comment after `export default` (on the keyword's line), with the
value authored on the next line (`export default /* c */⏎x`).

**tsv** reflows the value inline — the comment keeps its authored position trailing the
keyword, and only the author's line break is undone:

```
export default /* c */ x;
```

**Prettier** preserves the author's break, keeping the value on its own line at the
statement's own indent:

```
export default /* c */
x;
```

So the **comment position matches** — both trail `export default`; the only difference is
whether the author's line break survives. `prettier_variant_authored_break.svelte` carries
that broken form: prettier keeps it stable, tsv normalizes it back to `input.svelte`.

## Reason

**Design choice — uniformity.** A block comment does not run to end-of-line, so nothing
*forces* the value off the keyword's line; the break is ordinary layout, and tsv's TS
printer decides layout by width at every value position. Reflowing here is what makes this
gap agree with its twin `export =`
([export_equals_value_same_line_comment](../export_equals_value_same_line_comment_prettier_divergence/))
and with `const`/`let` `=`, `type =`, class properties, object values, parameter defaults,
arrow bodies, `:` annotations, return types, `satisfies`, and `as` — all of which reflow.

Contrast the **line**-comment case, where the break *is* forced (`//` runs to end-of-line)
and tsv indents the continuation one level: the
[Uniform Forced-Continuation Indent](../../../../../../docs/conformance_prettier.md#uniform-forced-continuation-indent).

Every authoring of this gap reaches the one inline fixed point, so they are variants here
rather than separate fixtures (a module allows only one `export default`, so each would
otherwise need its own file):

- `prettier_variant_authored_break.svelte` — value on the next line; prettier keeps it.
- `prettier_variant_blank.svelte` — a **blank** line between the comment and the value;
  prettier keeps that too. tsv collapses it, uniform with `x as /* c */⏎⏎T` — the one place
  tsv's Tier-1 blank-line significance yields, since the comment's own position already
  carries the signal.
- `unformatted_ours_own_line.svelte` — the comment authored on its **own** line with a blank
  below (`export default⏎/* c */⏎⏎x`). tsv reflows it inline; prettier pulls the comment onto
  the keyword line but keeps the break, landing on `prettier_variant_blank`. This authoring
  used to be its own fixture, sanctioned as a comment-*position* divergence; under the
  uniform rule tsv and prettier agree on the comment's position and only the break differs,
  so it folds in here.

See [conformance_prettier.md §Authored breaks in value position](../../../../../../docs/conformance_prettier.md#authored-breaks-in-value-position).
