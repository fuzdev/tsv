# inline_short_content_fill_long_prettier_divergence

A **short** inline element (its own content fits flat) sandwiched in a wrapping fill of text and
inline elements — the short-element counterpart of `inline_wide_content_trailing_long`. Two `<p>`
blocks, each a `<code>preferences</code> … <code>example.dev</code> …` run after a leading text line.

tsv packs the short element into the surrounding fill **greedily and pairwise**, exactly as prettier
does — `<code>example.dev</code>` stays on the run line after `set`/`sett`. It is **not** isolated onto
its own line: the flow boundary measures whether the *element* fits after the preceding word, not the
whole element-plus-trailing-text unit.

The two blocks pin the **100/101 boundary**:

- **Block 1 (100):** the run packs to exactly 100 columns (`…a little more`), so `text` breaks to its
  own line.
- **Block 2 (101):** one character more (`set` → `sett`) pushes `more` to column 101, so `more` breaks
  a word earlier (`…a little`, then `more text`).

The **sole divergence** is print width: prettier keeps each run on a **single over-width line** (the
inline-content hug, 101+ cols — see `output_prettier.svelte`), while tsv keeps printWidth a hard limit
and wraps at a word boundary.

```
tsv:       short element packs into the fill (≤100), wrapping at a word boundary
Prettier:  short element packs into the fill, but the run overflows printWidth
```

The `unformatted_ours_*` variants pin tsv's idempotence: a compact single-line run and an
extra-spaced run both normalize to `input.svelte` in one pass.

## Reason

Two deliberate choices, both shared with the wide-element sibling:

1. **printWidth is a hard limit** — tsv wraps the fill at a word boundary rather than emitting
   prettier's single over-width line. ◆print_width.
2. **A short inline element packs pairwise, like every other fill item** — it is not grouped with
   its trailing text into a unit that measures whole and drops before the element. When the element
   fits after the preceding word it packs there. This matches prettier's greedy fill and is the same
   pairwise measurement `inline_wide_content_trailing_long` relies on. ◆design_choice.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
