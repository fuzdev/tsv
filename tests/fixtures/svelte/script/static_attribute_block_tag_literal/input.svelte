<script foo="{#if a}b{/if}" bar="{@html c}"></script>

<!--
	The CONTROL for the sequence placement guard: a top-level `<script>`/`<style>` head is
	read by `read_static_attribute`, which runs no `read_sequence` at all, so every one of
	these values is literal text rather than a block or a tag out of place. See
	`svelte/attributes/value_block_tag_placement/` for the element reader, where the same
	spellings are rejected.
-->

<style baz="{:else}" qux="{/each}">
</style>
