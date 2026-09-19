# head_curried_chain_prettier_divergence

A curried arrow chain in a block head (`{#if}`, `{#each}`).

**tsv**: the head wraps like any other expression position. A chain too long for the line
breaks its heads — the first stays on the head's line and the rest indent under it, the
chain's default shape — and a chain whose heads force the break (a destructured parameter,
type parameters, a return type over parameters) breaks however short it is. The `}` drops
to the tag's column once the head breaks.

**Prettier**: never wraps a block head at all, so the chain prints on one line at any
width (past 140 columns for the long case here) and the forced break is flattened with it.

## Reason

The block-head rule of
[conformance_prettier_svelte.md §Svelte: Blocks](../../../../../docs/conformance_prettier_svelte.md#svelte-blocks):
print width is a hard limit in a head as everywhere else, the head's shape is keyed on
*that* it broke and not on why, and since prettier never wraps a head the geometry is
tsv's own — here simply the shape the same chain takes at every position with no chain
context of its own (`export default`, an array element). The first case is the null
control: a chain that fits with no break-forcing head is identical in both formatters.

The same chain as a `{@const}` value is not a divergence — prettier lays that tag out as an
assignment and tsv matches it
([const_curried_chain_long](../../tags/const/const_curried_chain_long/)).
