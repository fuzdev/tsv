<!--
	A global `svelte:*` element glued to content on BOTH sides stays on the content's line. The
	compiler hoists `<svelte:window>` / `<svelte:document>` / `<svelte:body>` / `<svelte:head>` out
	of the fragment before its whitespace rules run, so the two neighbours meet directly: a line
	break on either side would render as a space between them.
-->

<!-- text on both sides -->
text1<svelte:window on:resize={fn1} />text2

<!-- expression tags on both sides -->
{expr1}<svelte:document on:visibilitychange={fn2} />{expr2}

<!-- inline elements on both sides -->
<span>inline1</span><svelte:body on:mouseenter={fn3} /><span>inline2</span>

<!-- svelte:head with content, text on both sides -->
text3<svelte:head><title>text4</title></svelte:head>text5
