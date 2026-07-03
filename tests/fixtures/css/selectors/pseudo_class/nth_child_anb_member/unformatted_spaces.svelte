<style>
	/* An+B terms inside :nth-child() args are only recognized where terminated by
	   `)`/`,`/` of `; elsewhere the argument parses as an ordinary selector list, so
	   these structure like parseCss rather than one raw Nth value. */
	/* An+B in descendant position after a selector stays an Nth */
	:nth-child( .class1   2n ) {
		color: red;
	}
	:nth-child( div  span  2n ) {
		color: blue;
	}
	/* an An+B keyword not terminated by `)`/`,`/` of ` reads as a type selector */
	:nth-child(  even   odd  ) {
		color: green;
	}
	/* a comma makes the argument a selector list of An+B/selector terms */
	:nth-child(2n,.class1) {
		color: teal;
	}
	:nth-child(odd,even) {
		color: orange;
	}
</style>
