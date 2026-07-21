# inline_short_component_fill_long_prettier_divergence

The **component** sibling of `inline_short_content_fill_long` (which uses an HTML inline `<code>`). A
short inline **component** (its own content fits flat) sandwiched in a wrapping fill of text and inline
components. Two `<p>` blocks, each a `<Comp>preferences</Comp> … <Comp>example.dev</Comp> …` run after
a leading text line.

Components route through a different classification than HTML inline elements (a component is always
inline), but both project onto the **same** after-element fold — `build_after_element_fold` takes a
plain `DocId` and never asks the element's type — so a component packs into the surrounding fill
exactly as `<code>` does. tsv packs the short component **greedily and pairwise**, like prettier:
`<Comp>example.dev</Comp>` stays on the run line after `set`/`sett`. It is **not** isolated onto its
own line — the flow boundary measures whether the *component* fits after the preceding word, not the
whole component-plus-trailing-text unit.

The two blocks pin the **100/101 boundary** (identical widths to the sibling — `<Comp>` and `<code>`
are both 4-letter tags):

- **Block 1 (100):** the run packs to exactly 100 columns (`…a little more`), so `text` breaks to its
  own line.
- **Block 2 (101):** one character more (`set` → `sett`) pushes `more` to column 101, so `more` breaks
  a word earlier (`…a little`, then `more text`).

The **sole divergence** is print width: prettier keeps each run on a **single over-width line** (the
inline-content hug, 101+ cols — see `output_prettier.svelte`), while tsv keeps printWidth a hard limit
and wraps at a word boundary.

```
tsv:       short component packs into the fill (≤100), wrapping at a word boundary
Prettier:  short component packs into the fill, but the run overflows printWidth
```

The `unformatted_ours_*` variants pin tsv's idempotence: a compact single-line run and an
extra-spaced run both normalize to `input.svelte` in one pass.

## Reason

Two deliberate choices, both shared with the HTML-element sibling and the wide-element family:

1. **printWidth is a hard limit** — tsv wraps the fill at a word boundary rather than emitting
   prettier's single over-width line. ◆print_width.
2. **A short inline component packs pairwise, like every other fill item** — it is not grouped with
   its trailing text into a unit that measures whole and drops before the component. When it fits
   after the preceding word it packs there. This matches prettier's greedy fill and is the same
   pairwise measurement the HTML-element sibling relies on. ◆design_choice.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
