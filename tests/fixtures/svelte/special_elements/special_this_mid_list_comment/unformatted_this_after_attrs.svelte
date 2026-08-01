<!-- A `this` written after other attributes is hoisted to the head of the list; a comment -->
<!-- bound to another attribute keeps its place, matching prettier. Trailing an attribute: -->
<svelte:element data-attr="value" /* c1 */ this={x} />

<!-- Between two attributes -->
<svelte:element data-attr1="value1" /* c2 */ data-attr2="value2" this={x} />

<!-- Own-line, leading an attribute -->
<svelte:element
	/* c3 */
	data-attr="value"
	this={x}
/>
