<!--
	A frozen global `svelte:*` element glued to the content before it takes a line break after it
	whatever inline kind follows: a component, a `svelte:element`, a `svelte:boundary`.
-->

<!-- a component after -->
text1<!-- prettier-ignore --><svelte:window    on:x={fn1}   /> <Comp />

<!-- a svelte:element after -->
text2<!-- prettier-ignore --><svelte:document /> <svelte:element this={tag} />

<!-- a svelte:boundary after -->
text3<!-- prettier-ignore --><svelte:body /> <svelte:boundary>text3</svelte:boundary>

<!-- control: glued on both sides, the element stays on its neighbours' line -->
text4<!-- prettier-ignore --><svelte:head>  <title>text5</title>  </svelte:head>text6
