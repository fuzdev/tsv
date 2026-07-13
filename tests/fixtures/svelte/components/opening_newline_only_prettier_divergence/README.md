# opening_newline_only_prettier_divergence

A component whose content boundary was broken on **one** side only — a newline after `<Comp>`, but
none before `</Comp>`.

Content-boundary whitespace is render-free under Svelte 5 (whitespace at the start and end of a
tag's content is removed at compile), so a break on one side alone carries no expansion signal:
tsv's rule is that a component expands when the author broke **both** boundaries
([expr_child_multiline](../expr_child_multiline/), [multi_expressions_multiline](../multi_expressions_multiline/)),
and otherwise the content collapses back inline ([expr_child](../expr_child/),
[multi_expressions](../multi_expressions/)). The half-authored break lands in the second group.

Prettier instead honors the lone break and, having nothing to put before the closing tag, **dangles
its delimiter** (`{expr}</Comp⏎>`) so the content still hugs it. That is a third stable form for what
is the same document — the layout is being selected by a character the compiler deletes. tsv holds
to two forms (inline, or block-style with both tags intact) and never dangles *at a content
boundary*. (It does dangle at a **sibling** boundary — the `>` handoff to a following block — but
that one is keyed on inter-sibling whitespace, which Svelte 5 *keeps*. See
[conformance_prettier.md §Svelte: Blocks](../../../../../docs/conformance_prettier.md#svelte-blocks).)

`prettier_variant_opening_newline` is prettier's stable dangle form (tsv normalizes it to `input`);
the `unformatted_ours_*` variants are other authorings of the same document that tsv likewise
converges and prettier does not.

Supersedes the former `expr_child_opening_newline` and `multi_expressions_opening_newline` fixtures,
which pinned the dangle as tsv's own output.

See [conformance_prettier.md §Svelte: Inline content block-style](../../../../../docs/conformance_prettier.md#svelte-inline-content-block-style).
