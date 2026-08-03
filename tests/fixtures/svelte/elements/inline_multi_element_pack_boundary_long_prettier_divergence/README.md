# inline_multi_element_pack_boundary_long_prettier_divergence

The exact width boundary of the mid-run pack that `inline_multi_element_pack_long` covers at
the shape level, inside an **inline** container. Two root `<span>`s, each a
`text… <code>inline1</code> kilo lima <code>inline2</code> tail…` run: text after the first
element packs onto its line, and the second element folds with its **terminal** trailing text
on the line after.

The two spans pin the **100/101 boundary**:

- **Span 1 (100, control):** the run packs to exactly **100 columns** (`…kilo lima`), so the
  second element and its tail take the next line. Prettier produces the same form — the pack
  itself is agreement, not divergence.
- **Span 2 (101):** one character more in the lead (`abcd` → `abcde`) pushes `lima` past 100,
  so `lima` moves to the second line and the element folds with its tail **after** it
  (`lima <code>inline2</code> mike november oscar papa`).

The **divergence is at that second line**: prettier also holds `prettier_variant_own_line`
stable — the same document authored with a newline between `lima` and the element, which leaves
`lima` **isolated** on a line of its own and drops the element plus tail to a third line. tsv
converges both authorings onto one form, because the separator's *spelling* between a text
sibling and an element carries no signal (space and newline render identically), so the word
and the element it precedes reflow as one fill.

```
tsv:       lima <code>inline2</code> mike november oscar papa    (one form per document)
Prettier:  lima                                                  (own-line authoring kept)
           <code>inline2</code> mike november oscar papa
```

`unformatted_ours_compact` pins tsv's convergence from the other side: a single-line authoring
normalizes to `input.svelte`, where prettier dangles the tag delimiters instead.
`unformatted_spaces` (an extra-spaced run) normalizes to input under **both** formatters, so
the divergence is confined to the element↔text boundary, not the run-interior reflow.

## Reason

Two deliberate choices, both already carried by this fixture's siblings:

1. **The pack measures the element pairwise, not the element-plus-trailing-text unit.** The
   word before an element and the element itself share one fit check; the element's fold with
   its terminal tail is a separate decision measured from the line the element lands on. That
   is what puts the whole fold on one line at 101 instead of isolating the preceding word —
   the same pairwise measurement `inline_short_content_fill_long` relies on. ◆design_choice.
2. **A separator's presence carries signal, its spelling does not.** Svelte 5 collapses an
   inter-sibling whitespace run to one whitespace, so `lima <code>` and `lima⏎<code>` are the
   same document and reach one fixed point. Prettier keeps a distinct stable form for each
   authoring. ◆design_choice.

See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
