<!--
	The boundary after a frozen global `svelte:*` element is the printer's wherever the directive
	sits and whatever the element spans: the whitespace after it is a line break.
-->

<!-- the directive on its own line, prose after -->
<!-- prettier-ignore -->
<svelte:window    on:x={fn1}   />
text1 {expr}

<!-- another global element before the directive -->
<svelte:body />
<!-- prettier-ignore --><svelte:document    on:x={fn2}   />
text2

<!-- a svelte:head that spans lines -->
text3<!-- prettier-ignore --><svelte:head>
	<title>text4</title>
</svelte:head>
text5
