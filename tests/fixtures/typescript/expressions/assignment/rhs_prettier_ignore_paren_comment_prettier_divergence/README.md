# rhs_prettier_ignore_paren_comment_prettier_divergence

The assignment-RHS host of
[init_prettier_ignore_paren_comment](../../../statements/variable/init_prettier_ignore_paren_comment_prettier_divergence/):
an own-line `prettier-ignore` in the `=`→RHS gap freezes the value, and a comment the
author wrote between the frozen slice and the `)` that closes the shell around it.

The freeze slice is the value's node span, so the shell lies outside it — and whether the
shell is retained, where the comment renders, and whether it defers past the `;` are
questions about the **gap**, not about what renders between the parens. The frozen and
unfrozen forms therefore answer identically, which is the whole claim: a declarator
initializer and an assignment RHS are one rule at two hosts (the pairing
[rhs_prettier_ignore_head](../rhs_prettier_ignore_head/) and
[init_prettier_ignore_head](../../../statements/variable/init_prettier_ignore_head/)
already make for the unparenthesized value).

## Why tsv differs

**Prettier throws on this input** — its every-comment-printed assertion fires:

```
Comment "c" was not printed. Please report this error!
```

Its `prettier-ignore` handling replaces the RHS with the verbatim source range and never
visits the comment past that range's end, while the shell around it still prints. There is
no prettier output to compare against, which is what `prettier_rejects.txt` records — a
prettier release that fixes the bug fails this fixture and flags the case for promotion.

## Expected behavior

- **tsv**: a `//` retains the shell and stays inside it, the closer on its own line; a
  sequence's own required parens hold the comment the same way; a shell the value does not
  need strips and the block defers past the `;`. The input is a fixed point.
- **acorn**: accepts, so `expected.json` is an ordinary oracle file.
- **prettier**: throws; no formatted output exists.

`unformatted_ours_paren` is the authoring all three start from — the third case still
parenthesized, which is where its comment had no emitter at all. That third case's own
prettier oracle *does* exist (prettier only throws when the shell survives) and is pinned
at the declarator host, by
[init_prettier_ignore_redundant_paren_comment](../../../statements/variable/init_prettier_ignore_redundant_paren_comment_prettier_divergence/).

## Reason

◆comment_preservation ◆prettier_bug — prettier crashes on valid input tsv formats stably,
and the placement tsv chooses is the authored one. Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On assignment-family value heads*) and listed in
[conformance_prettier_ts.md §Prettier rejects valid input](../../../../../../docs/conformance_prettier_ts.md#prettier-rejects-valid-input);
the governing principle is
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy).
