# annotation_leading_block_prettier_divergence

A block comment between `:` and the type in a property signature has two
authored-intentional stable positions, and both formatters preserve each
when given as input:

- Form A (inline, canonical input): `a: /* block */ X;`
- Form B (before the `:`, dual-stable): `a /* block */: X;`

The divergence is in how the formatters **normalize the unstable layouts**
that arise when a user breaks the line around the comment:

- Input C — `a: /* block */\n  X;` (block on `:` line, type on next):
  - tsv: → form A (`a: /* block */ X;`)
  - prettier: → form B (`a /* block */: X;`)

- Input D — `a:\n  /* block */\n  X;` (block fully on its own line):
  - tsv: → form A
  - prettier: → form C (`a: /* block */\n  X;`), which then takes another
    pass to converge to form B. The unstable form-C output of the first
    pass is captured as
    `prettier_intermediate_to_variant_block_own_line.svelte`.

In both unstable cases, prettier eventually relocates the block before the
`:`; tsv compacts to the inline position after the `:`.

## Reason

Form A and form B express different user intent — block annotating the
**value** vs block annotating the **key**. Both formatters respect that
intent when the input is unambiguous.

For ambiguous inputs (block sandwiched between newlines), the formatters
pick different canonical targets. tsv chooses form A because it keeps the
block on the value side, matching the user's choice to write `:` first;
prettier chooses form B, treating the block as trailing on the name.

Neither choice is information-destructive (the block text and its
position-relative-to-the-colon are both preserved in either canonical
form). This divergence is purely about which stable form to favor for
unstable inputs.

Reason: Comment normalization (stable quirks). See
[conformance_prettier.md](../../../../../../docs/conformance_prettier.md)
§Comment normalization (stable quirks).
