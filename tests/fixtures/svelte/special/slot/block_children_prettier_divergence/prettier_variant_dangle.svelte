<!--
	A <slot> whose children are block elements goes multiline and lays out block-style — both tags
	intact, content on its own indented lines — exactly like a regular element in the same shape.
	The hugged content boundary is render-free under Svelte 5, so it does not select the layout;
	prettier reads it as an instruction to keep the content glued to the tags and dangles both
	delimiters — see prettier_variant_dangle.
-->
<slot
	><div>block1</div>
	<div>block2</div></slot
>

<!-- A regular inline element in the same shape, for parity -->
<span
	><div>block1</div>
	<div>block2</div></span
>
