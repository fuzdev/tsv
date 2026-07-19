# inline_empty_long_prettier_divergence

An inline element with **whitespace-only content** and attributes long enough to wrap. The
content is render-free (Svelte trims every fragment edge at compile, and a whitespace-only
fragment renders nothing), so tsv treats the element as empty: the attributes wrap and the
close hugs (`></span>`), identical to the truly-empty authoring. Prettier preserves the
space as content, which forces `>` and `</span>` onto separate lines.

- `unformatted_ours_spaces.svelte` — the whitespace-content authoring
  (`<span data-attr="…">   </span>`); tsv normalizes it to `input.svelte`, prettier does
  not.
- `prettier_variant_ws.svelte` — prettier's stable form of it: attributes wrapped, `>` and
  `</span>` on separate lines. tsv normalizes it to `input.svelte`.
- `unformatted_compact.svelte` — one-line authorings; both formatters wrap the attributes
  the same way.

## Reason

Svelte-mirror whitespace: whenever tsv keeps content inline, every whitespace character in
the output is one the compiler keeps — a whitespace-only fragment keeps none. See
[conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).

## Related

- [inline_boundary_whitespace](../inline_boundary_whitespace_prettier_divergence/) — the
  fits-inline boundary trim, including whitespace-only elements with short attributes
