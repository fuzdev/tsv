# trailing_member_after_call_comment_prettier_divergence

A line comment after a call in a member chain, before the trailing `.length`
access. The compact form `items.filter((x) => x).length; // after filter` is stable
in both formatters (`compare` matches on `input.svelte`); the divergence is in how
each normalizes the **expanded** authoring (`unformatted_ours_expanded`, where the
comment dangles after the call before `.length`):

- **tsv**: collapses the chain back to the compact `input.svelte`, trailing the
  comment past the `;`.
- **Prettier**: keeps the chain expanded, dangling the comment after `)` before
  `.length` (`prettier_variant_dangle`) — a stable form prettier preserves but tsv
  normalizes to `input`.

Per Comment Position Philosophy, both keep the comment associated with the same
position in the chain; the divergence is purely the inline-vs-expanded chain
layout. This is the member-access sibling of the computed-access
`trailing_member_computed_comment` divergence.

See [conformance_prettier.md §Comment normalization (stable quirks)](../../../../../../../docs/conformance_prettier.md#comment-normalization-stable-quirks).
