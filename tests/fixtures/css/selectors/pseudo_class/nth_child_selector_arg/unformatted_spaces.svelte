<style>
	/* Non-An+B :nth-child arguments parse as ordinary selectors (drop-in parity
	   with parseCss), not as a single raw Nth value. These are semantically invalid
	   CSS, but a drop-in parser must still produce parseCss's structured AST. */
	:nth-child(  .class1  ) {
		color: red;
	}
	:nth-child( div.class1 ) {
		color: blue;
	}
	:nth-child(  #id1  ) {
		color: green;
	}
	:nth-child( a:hover ) {
		color: teal;
	}
	:nth-child(.class1,.class2) {
		color: orange;
	}
	:nth-child(.class1  >  .class2) {
		color: purple;
	}
</style>
