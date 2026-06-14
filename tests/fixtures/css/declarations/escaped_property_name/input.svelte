<style>
	div {
		/* a hex escape consumes one following whitespace as its terminator; that
		   space is part of the property-name token, so it's kept before the colon */
		\41 : red;
		/* no terminator whitespace — the colon ends the escape, so no space is added */
		\43: green;
		/* terminator preserved when the escape ends a longer property name */
		ab\44 : blue;
		/* custom property names keep the terminator too */
		--\45 : teal;
	}
</style>
