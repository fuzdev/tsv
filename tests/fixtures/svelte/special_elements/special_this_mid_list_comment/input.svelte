<!-- A `this` written after other attributes is hoisted to the head of the list; a comment -->
<!-- bound to another attribute keeps its place, matching prettier. Trailing an attribute: -->
<svelte:element this={x} data-attr="value" /* c1 */ />

<!-- Between two attributes -->
<svelte:element this={x} data-attr1="value1" /* c2 */ data-attr2="value2" />

<!-- Own-line, leading an attribute -->
<svelte:element
	this={x}
	/* c3 */
	data-attr="value"
/>
