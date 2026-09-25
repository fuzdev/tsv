<!--
	A `<svelte:head>` with content, glued to text on both sides, at the widest that still takes
	the closing-`>` dangle: the dangle line is exactly 100 wide. One character more and the
	content goes block-style instead (global_glued_both_sides_wide_long_prettier_divergence).
-->
text1<svelte:head><title>text2</title><meta name="value1" content="value2value2valu" /></svelte:head
>text3
