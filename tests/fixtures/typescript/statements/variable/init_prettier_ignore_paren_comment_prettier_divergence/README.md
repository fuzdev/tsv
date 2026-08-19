# init_prettier_ignore_paren_comment_prettier_divergence

A comment the author wrote between a **frozen** initializer and the `)` that closes the
pair the printer prints around it — an own-line `prettier-ignore` in the `=`→initializer
gap freezes the value, and the value still takes a paren pair on the way out: an
assignment's clarity parens (`const a = (b = c)`) or a sequence's own required ones.

The freeze slice is the value's node span, so those parens sit **outside** it and this gap
is inside a pair the printer synthesizes. Nothing enclosing can see between them, so the
gap belongs to the initializer's own shell emitter.

## Why tsv differs

**Prettier throws on this input** — its every-comment-printed assertion fires:

```
Comment "c" was not printed. Please report this error!
```

Its `prettier-ignore` handling replaces the value with the verbatim source range and never
visits the comment sitting past that range's end, while the pair around it still prints. So
there is no prettier output to format against, which is what `prettier_rejects.txt` records
(its trimmed content is the expected-error substring, so a prettier release that fixes the
bug fails this fixture and flags the case for promotion to an ordinary one).

tsv keeps the comment where the author wrote it — inside the pair, after the slice — which
is the answer its **unfrozen twins** already give at the identical gap:
[init_assignment_paren_block_comment](../init_assignment_paren_block_comment/) (a block
stays inline inside the parens, matching prettier) and
[init_assignment_paren_line_comment](../init_assignment_paren_line_comment_prettier_divergence/)
(a `//` opens the pair, because inline it would swallow the `)`). The freeze changes what
renders between the parens, not where the gap's comment goes.

## Expected behavior

- **tsv**: the comment stays inside the pair; the block form stays inline, the line form
  breaks the pair open with the closer on its own line. The input is a fixed point.
- **acorn**: accepts, so `expected.json` is an ordinary oracle file — the divergence is
  with prettier alone.
- **prettier**: throws; no formatted output exists.

`unformatted_ours_compact` glues the comment to the value and folds the broken pair back
up; `unformatted_ours_double_paren` wraps a second, redundant shell around the pair, which
collapses into the single pair the printer emits, comment and all. Both normalize to
`input.svelte` under tsv, and neither has a prettier answer to compare against.

## Reason

◆comment_preservation ◆prettier_bug — prettier crashes on valid input tsv formats stably,
and the placement tsv chooses is the authored one. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*) and listed in
[conformance_prettier_ts.md §Prettier rejects valid input](../../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input);
the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).

The sibling gap where the pair does **not** survive — a redundant shell the freeze strips —
has its own fixture,
[init_prettier_ignore_redundant_paren_comment](../init_prettier_ignore_redundant_paren_comment_prettier_divergence/),
where prettier does produce output.
