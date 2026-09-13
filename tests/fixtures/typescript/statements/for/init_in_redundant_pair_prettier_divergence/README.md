# init_in_redundant_pair_prettier_divergence

An `in` binary lexically under a `for` header's init is parenthesized so the header does not
read as a for-in head. Two positions under that init already carry a pair of their own, and
there tsv prints **one pair fewer** than prettier:

```ts
// tsv                                    // prettier
for (!(/* c */ 'aaa' in bbb); ;) {}       for (!(/* c */ ('aaa' in bbb)); ;) {}
for (fn((lll, 'aaa' in bbb)); ;) {}       for (fn((lll, ('aaa' in bbb))); ;) {}
```

At the unary operand the pair is the printer's own **comment-holder shell**, which the block
comment forces open; it already parenthesizes the `in`, so nothing needs a second one, and
prettier adds one inside it. In the call argument no pair is needed at all — a call argument
is `[+In]` in its own right — and tsv keeps the sequence's grouping parens alone where
prettier wraps whichever operand holds the `in`.

Neither is a freeze question: both reproduce with no directive anywhere, and both are
pre-existing. Both outputs parse (tsc accepts each), and both formatters are idempotent on
their own.

## Expected behavior

- **tsv**: one pair per position — the shell's at the unary operand, the sequence's own in the
  call argument.
- **prettier**: a second pair around the `in` itself (`output_prettier.svelte`).

The controls carry the shapes that agree: the same unary operand written **without** the
comment takes no shell, so both tools print the single ambient pair; and a clause that IS the
`in` takes that pair at its own position under both (bare, the header reads as a for-in head
and does not parse).

## Reason

◆design_choice — a representative disagreement over a redundant pair, not a validity one.
Sanctioned in
[conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive)
(under *On delimiter-owned value heads, and on sequence operands*, the `[~In]` paragraphs);
the frame is
[conformance_prettier.md §Decision framework](../../../../../../docs/conformance_prettier.md#decision-framework).
