# range_glued_prettier_divergence

A `prettier-ignore-start` / `-end` range is **byte-verbatim** in tsv: the slice between the
markers is the author's source, untouched. Prettier instead freezes each node's *content*
(raw attributes survive) but re-lays out the whitespace **between** nodes with its ordinary
block/inline rules. The two part wherever that re-layout changes something — glued block
siblings, and content glued to a marker:

```svelte
<!-- prettier-ignore-start -->
<p>block1</p><p>block2</p>
<!-- prettier-ignore-end -->
```

tsv keeps the glue; prettier splits the `<p>`s onto their own lines. Same for a node glued
to either marker (`<!-- prettier-ignore-start --><div …>`). Glued **inline** siblings
(`<span>inline1</span><span>inline2</span>`) are kept by both tools — there the glue is
render-significant, so even prettier's re-layout must preserve it.

## Reason

The range's promise is "these bytes are mine". Inter-node whitespace inside the fence is as
much the author's as the nodes are, and a re-layout that must stop at every render-sensitive
seam anyway (the inline case) buys nothing over not touching the region at all. tsv's form
is also self-consistent with the single-node `prettier-ignore`, which both tools already
emit raw.

The same stance decides the seam left behind when a hoisted `<script>` / `<style>` /
`<svelte:options>` is cut out of a range (see [range_section_hoist](../range_section_hoist/)):
the cut removes the section's bytes plus the whitespace run immediately before it, and what
remains is joined verbatim — glued neighbours stay glued
(`unformatted_ours_section_in_range.svelte`, which tsv normalizes to `input.svelte` while
prettier re-lays out the seam). Since a hoisted section renders nothing in place, the glue
preserves the authored render exactly; a fabricated newline between inline siblings would
change it.

See [conformance_prettier_ignore.md §Format-ignore directive](../../../../../../docs/conformance_prettier_ignore.md#format-ignore-directive).
