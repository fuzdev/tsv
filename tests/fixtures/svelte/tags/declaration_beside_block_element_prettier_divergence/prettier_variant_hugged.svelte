<!--
	A declaration ({@const}, {const}, {let}, {#snippet}) takes its own line unless it is glued to
	content on both sides. A block element beside it is not that content: the block element owns
	its own line, so a declaration between a block element and content (text, an expression tag,
	an inline element) is glued on one side only and takes its line, the content beside it too.
	Prettier keeps this form once written, but keeps a compact declaration glued to the content
	beside it.
-->

<!-- a {#snippet} between a block element and text -->
<b
	><div>{y}</div>
	{#snippet s()}x{/snippet}t</b
>

<!-- a {@const} between a block element and text, in a component -->
<Comp
	><div>{y}</div>
	{@const z = 1}t</Comp
>

<!-- an inline element after it -->
<Comp
	><div>{y}</div>
	{@const z = 1}<i>x</i></Comp
>

<!-- an expression tag after it -->
<Comp
	><div>{y}</div>
	{@const z = 1}{w}</Comp
>

<!-- the text first -->
<Comp
	>t{@const z = 1}
	<div>{y}</div></Comp
>

<!-- an inline element first -->
<Comp
	><i>x</i>{@const z = 1}
	<div>{y}</div></Comp
>

<!-- a {const} at the root -->
<div>{y}</div>
{const z = 1}t

<!-- a {let} in an inline element -->
<b
	><div>{y}</div>
	{let z = 1}t</b
>

<!-- in a block parent -->
<div>
	<div>{y}</div>
	{#snippet s()}x{/snippet}t
</div>

<!-- in a block body -->
{#if c}<p>{y}</p>
	{@const z = 1}t{/if}

<!-- a block element too wide for its line -->
<b
	><div
		class="cccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccccc"
	>
		{y}
	</div>
	{#snippet s()}x{/snippet}t</b
>

<!-- control: glued to text on both sides, it stays glued -->
<Comp>a{@const z = 1}b</Comp>
