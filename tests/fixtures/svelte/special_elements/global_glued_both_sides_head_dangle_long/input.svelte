<!--
	A `<svelte:head>` with content, glued to text on both sides, at 101 wide: the closing `>`
	dangles onto the following text's line, and both glued boundaries stay unbroken.
-->
text1<svelte:head><title>text2</title><meta name="value1" content="value2value" /></svelte:head
>text3
