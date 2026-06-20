# void_element_body_long_prettier_divergence

How tsv lays out a `{#key}` whose body is a **void/empty element** (`<Spinner />` — no
attributes, no children) as the head grows, and why it differs from a *breakable*
element. The cases (in `input.svelte`):

1. **@100** — head + body = 100: fits fully inline (`}<Spinner />{/key}`).
2. **@101** — head + body > 100 but the head fits on its own line: the void element
   **can't break internally**, so it's **atomic** — the head stays flat and the body
   drops to its own line (the one-pass middle-zone layout). This is the 100/101 boundary.
3. **head alone > 100** — the head wraps *and* the body expands.
4. **contrast** — `<Spinner class="…" />` (has an attribute, so it *can* break): it
   **hugs** the head and wraps its own attribute, rather than dropping to its own line.

The divergence has two parts:

- **Normalization (the void-atomic point).** From a compact one-line input
  (`unformatted_ours_compact.svelte`), tsv expands a void body onto its own line, while
  prettier keeps it on the head line and breaks the self-closing `/>`
  (`prettier_variant_compact.svelte`). Both keep tsv's canonical form stable, so this
  shows only when normalizing the compact form — and it's the 1-pass idempotency guard
  for the body classification (a void element must take the atomic `conditional_group`
  path, not the `if_break` hug path, or it would wrap-then-unwrap across two passes).
- **Head wrap (`output_prettier.svelte`).** Case 3's head wraps in tsv but stays inline
  in prettier — the standard block-head divergence (see `key/long`).

## Reason

A void/empty element has no attributes or children, so it has no internal break point to
absorb width — like text and expression tags, it's atomic. An element *with* attributes
or children does have one and hugs the head instead. See
[conformance_prettier.md §Svelte: Blocks](../../../../../../docs/conformance_prettier.md#svelte-blocks).

## Related

- [key/long](../long_prettier_divergence/) — the standalone head-wrap + dangle + body-expand divergence
- [snippet/inline_element_long](../../snippet/inline_element_long_prettier_divergence/) — another middle-zone (params inline + body expand) divergence
