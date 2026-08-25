text1 text2
<div>block1</div>

<!--
	A block element after spaced text takes its own line, whether the text is the fragment's
	first child (above, at the root) or not, in every fragment family. Prettier hugs the block to
	a FIRST-child text (`text1 text2 <div>block1</div>`) and breaks it after any other
	predecessor (`prettier_variant_space.svelte`).
-->

<!-- a first-child text before a short block, in a block element, a section, an inline element,
     a component and a block body -->
<div>
	text1 text2
	<div>block1</div>
</div>
<section>
	text1 text2
	<p>block1</p>
</section>
<span>
	text1 text2
	<div>block1</div>
</span>
<Comp>
	text1 text2
	<div>block1</div>
</Comp>
{#if cond}
	text1 text2
	<div>block1</div>
{/if}

<!-- a one-word label before the block, and text on both sides of it -->
<div>
	text1
	<div>block1</div>
</div>
<div>
	text1 text2
	<div>block1</div>
	text3 text4
</div>

<!-- a block that renders multiline: its head never ends the text line -->
<div>
	text1 text2
	<div>
		<p>block1</p>
	</div>
</div>
<Comp>
	text1 text2
	<div>
		<p>block1</p>
	</div>
</Comp>

<!-- controls, where both formatters break already: a text that is not the fragment's first
     child (after an element, after a tag), and an element predecessor -->
<div>
	<span>inline1</span> text1 text2
	<div>block1</div>
</div>
<div>
	{expr} text1 text2
	<div>block1</div>
</div>
<div>
	<span>inline1</span>
	<div>block1</div>
</div>
