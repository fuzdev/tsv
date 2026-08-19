{#if aa}
	{@const bb =
		/** @type {A} */
		(cc)}
	text1

	<!-- the freeze does not move the cast either: the frozen slice starts at the cast's
	`(`, so the slice owes the comment its claim — and the author's line for it -->
	{@const dd =
		// prettier-ignore
		/** @type {A} */
		(ee  +  ff)}

	<!-- the glued authoring is the other one, and it stays glued frozen too -->
	{@const gg =
		// prettier-ignore
		/** @type {A} */ (hh  +  ii)}
{/if}
