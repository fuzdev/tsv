<style>
	/* U+00A0 between the An+B term and the `)` is a boundary run the parser skips;
	   it is kept flush against the `)` */
	:nth-child(2n + 1 ) {
		color: red;
	}

	/* a keyword term takes the same tail */
	:nth-child(even ) {
		color: blue;
	}

	/* both sides of the term at once */
	:nth-child( 2n + 1 ) {
		color: green;
	}

	/* the same run in a non-nth functional pseudo */
	:is(2n ) {
		color: teal;
	}
</style>
