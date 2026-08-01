<!-- A `this` written after other attributes still prints first, and a comment bound to a -->
<!-- token that stays put stays with it. An inline comment after the tag name keeps the -->
<!-- head (prettier re-glues it to the attribute that followed it) -->
<svelte:element /* c1 */ data-attr="value" this={x} />

<!-- A line comment on the tag-name line stays there (prettier gives it its own line) -->
<svelte:element // c2
	data-attr="value"
	this={x}
/>

<!-- An own-line comment leading `this` travels with it (prettier moves it to the end of the list) -->
<svelte:element
	data-attr="value"
	/* c3 */
	this={x}
/>

<!-- The component's synthesized `this` hoists the same way -->
<!-- (prettier re-glues the comment to the prop that followed it) -->
<svelte:component /* c4 */ prop="value" this={Comp} />
