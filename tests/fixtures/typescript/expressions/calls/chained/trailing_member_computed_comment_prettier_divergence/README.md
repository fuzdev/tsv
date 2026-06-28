# trailing_member_computed_comment_prettier_divergence

A line comment before a computed member access (`// comment\n[0]`) on a call
chain. The compact form `items.filter((x) => x)[0]; // comment` is stable in both
formatters (`compare` matches on `input.svelte`); the divergence is in how each
formatter normalizes the **expanded** authoring (`unformatted_ours_expanded`,
where the comment sits on its own line before `[0]`):

- **tsv**: normalizes the expanded form back to the compact `input.svelte` in one
  pass.
- **Prettier**: does not collapse it. Its first pass produces the unstable
  `prettier_intermediate_to_variant_expanded` (`[// comment\n0]`), and a second
  pass settles on `prettier_variant_dangle` (the call breaks across lines with the
  comment dangling after `)` before `[0]`) — a stable form prettier keeps but tsv
  normalizes to `input`.

Chain: `unformatted_ours_expanded` → prettier → `prettier_intermediate_to_variant_expanded`
(unstable) → prettier → `prettier_variant_dangle` (stable).

## Reason

Prettier keeps the call-chain comment in its expanded position rather than
collapsing the chain; tsv prefers the compact single-line form. Per Comment
Position Philosophy, both keep the comment associated with the same access — the
divergence is purely the inline-vs-expanded chain layout.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
