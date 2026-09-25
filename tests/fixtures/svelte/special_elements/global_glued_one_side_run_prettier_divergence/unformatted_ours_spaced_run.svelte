<!--
	Global `svelte:*` elements glued to text on one side only: a run of two with whitespace between
	its elements, and a `<svelte:head>`. Nothing is glued on both sides, so each takes its own line.
-->
text1<svelte:window on:x={fn1} /> <svelte:body on:x={fn2} />text2 <svelte:head><title>text3</title></svelte:head>text4
