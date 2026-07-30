<style>
	div {
		/* an empty element between two commas is a list entry, and the list breaks one per line */
		transition: a,,b;
		/* a leading empty element */
		transition: ,a;
		/* inside a function the list stays inline */
		background: linear-gradient(red,,blue);
		/* a custom property is exempt from the one-per-line break */
		--x: a,,b;
	}
</style>
