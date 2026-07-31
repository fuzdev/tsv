<!--
	`<svelte:options>` configures the component instead of rendering, so it fills the
	root's options slot rather than becoming a node in the fragment. Only a direct child
	of the root can do that — nested inside an element or a block it is a parse error
	(see the input_invalid_* files).
-->
<svelte:options runes />

<div>block1</div>

{#if cond}
	<div>block2</div>
{/if}
