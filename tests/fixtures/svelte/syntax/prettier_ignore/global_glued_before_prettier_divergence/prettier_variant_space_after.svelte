<!--
	A frozen global `svelte:*` element glued to the content before it, with whitespace after it,
	lays out on that side as the same element unfrozen does: the whitespace after it is a line
	break. The compiler hoists the element out of the fragment, so its neighbours meet across that
	whitespace and render one space either way; the directive glued in front of it stays glued.
-->

<!-- text before, text after, svelte:head with content -->
text1<!-- prettier-ignore --><svelte:head>  <title>text2</title>  </svelte:head> text3

<!-- an expression tag before, an inline element after -->
{expr}<!-- prettier-ignore --><svelte:document    on:x={fn1}   /> <span>inline1</span>

<!-- an inline element before, text after, the tool-neutral spelling -->
<span>inline2</span><!-- format-ignore --><svelte:body on:x={fn2} /> text4

<!-- a control-flow block before -->
{#if cond}text5{/if}<!-- prettier-ignore --><svelte:window    on:x={fn3}   /> text6
