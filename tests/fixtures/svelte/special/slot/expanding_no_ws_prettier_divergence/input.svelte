<!-- A <slot> whose only child is an expanding block ({#if}/{#each}/{#key}) with no boundary
     whitespace. The block forces the slot multiline, and it lays out block-style — both tags intact,
     content on its own indented line — exactly like a regular element in the same shape
     (../../../elements/inline_with_if_block_prettier_divergence/). The hugged boundary is render-free
     under Svelte 5, so it does not select the layout; prettier reads it as an instruction to keep the
     content glued to the tags and dangles both delimiters — see prettier_variant_dangle. -->
<slot>
	{#if cond}text{/if}
</slot>

<slot name="x">
	{#each items as item}{item}{/each}
</slot>

<slot>
	{#key id}text{/key}
</slot>
