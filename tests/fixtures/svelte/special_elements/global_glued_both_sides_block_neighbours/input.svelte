<!--
	A control-flow block beside a global `svelte:*` element is content, like a text: with the
	text on the other side glued too, the element stays on their line.
-->

<!-- an if block before -->
{#if cond}text1{/if}<svelte:window on:x={fn1} />text2

<!-- an each block after -->
text3<svelte:document on:x={fn2} />{#each items as item}{item}{/each}

<!-- a key block before -->
{#key expr}text4{/key}<svelte:body on:x={fn3} />text5

<!-- a comment after, glued, with the text on the next line -->
text6<svelte:head><title>text7</title></svelte:head><!-- comment -->
text8
