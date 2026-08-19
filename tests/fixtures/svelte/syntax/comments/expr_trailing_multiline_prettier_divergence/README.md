# expr_trailing_multiline_prettier_divergence

The **multi-line block** counterpart of
[expr_trailing](../expr_trailing_prettier_divergence/): prettier drops trailing comments
in template expressions; tsv preserves them — and renders the preserved comment through
the TS printer's multi-line block form, exactly as the same expression formats in a
`<script>` block. For a `*`-aligned comment (every interior line begins with `*`) that
reindents the interior lines to the expression's own context (one leading space before
the `*`); a non-`*`-aligned interior is preserved verbatim.

tsv (the `input.svelte` fixed point, at the tag's own indent):

```svelte
{a /** c1
 * c2
 */}
```

Prettier: `{a}` — the comment is lost.

Contexts covered: `{expr}` (top-level and nested one indent in), `{#if}` (whose closing
`}` takes its own line below the comment), `bind:value={}` (the value expands and the
interior reindents to the value's depth), `{@debug}`, `{@const}`, and the
non-`*`-aligned control.

Prettier's `{@const}` output is additionally **corrupt** — it emits an unmatched paren
(`{@const y = item) /** c1`), then throws on its own output; notably the comment
interior in those corrupt bytes IS reindented, prettier's JS printer applying the same
multi-line block form tsv uses. The committed `output_prettier.svelte` records the
first-pass bytes verbatim; there is no `audit_signature.txt` because prettier cannot
take a second pass over them. Same bug as the `{@const}` cases in
[expr_trailing_line](../expr_trailing_line_prettier_divergence/) and
[expr_trailing_indented_content](../expr_trailing_indented_content_prettier_divergence/).

The `unformatted_ours_flat.svelte` / `unformatted_ours_indented.svelte` variants author
the same comments with flat (column-0) and over-indented interiors; tsv normalizes both
to `input.svelte`'s aligned form — the same normalization the comment gets from
`tsv_ts` in a `<script>` block — while prettier drops the comments instead, so the
variants carry the divergence.

## Reason

User comments are valuable and shouldn't be silently removed; the comments are
syntactically valid here. Preservation is the family's rule
([expr_trailing](../expr_trailing_prettier_divergence/)); what this fixture adds is the
*rendering* of the preserved multi-line comment: TypeScript formatting is context-free
in tsv, so the trailing comment takes the identical multi-line block form (reindent for
`*`-aligned, verbatim otherwise) it takes in a standalone script. Template-**tag**
comments (between attributes) are a different region with a live prettier oracle and
deliberately stay verbatim — see
[attributes/comment](../../../attributes/comment/). See
[conformance_prettier.md §Comment Position Philosophy](../../../../../../docs/conformance_prettier.md#comment-position-philosophy);
cataloged in
[conformance_prettier_svelte.md §Svelte: Attributes](../../../../../../docs/conformance_prettier_svelte.md#svelte-attributes).

## Related

- [expr_trailing_line](../expr_trailing_line_prettier_divergence/) — the `//` sibling
- [paren_multiline_comment](../../../expression_tag/paren_multiline_comment_prettier_divergence/) —
  the *leading* multi-line block, which already takes the reindented form
