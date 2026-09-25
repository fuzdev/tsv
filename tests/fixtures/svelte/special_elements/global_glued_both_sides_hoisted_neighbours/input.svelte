<!--
	A hoisted neighbour is not content, so the glue is asked past it: each global `svelte:*`
	element below is glued to text on both sides through a declaration tag, a `{@debug}` or a
	`{#snippet}`, and the whole run stays on the text's line, the hoisted neighbour included.
-->

<!-- a declaration tag before -->
text1{const a = 1}<svelte:window on:x={fn1} />text2

<!-- a declaration tag after -->
text3<svelte:document on:x={fn2} />{let b = 1}text4

<!-- a debug tag before -->
text5{@debug a}<svelte:body on:x={fn3} />text6

<!-- a snippet before -->
text7{#snippet fn4()}text8{/snippet}<svelte:head><title>text9</title></svelte:head>text10
