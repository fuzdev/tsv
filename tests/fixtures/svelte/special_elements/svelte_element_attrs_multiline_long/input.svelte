<!-- short attrs, multiline children - stays inline -->
<svelte:element this={tag} class="value">
	<div>text</div>
</svelte:element>

<!-- long attrs, multiline children - > on own line -->
<svelte:element
	this={tag}
	data-attr1="aaaaaaaaaa"
	data-attr2="bbbbbbbbbb"
	data-attr3="cccccccccc"
	data-attr4="dddddddddddddddddddddddddddddddddddddddddddddd"
>
	<div>text</div>
</svelte:element>
