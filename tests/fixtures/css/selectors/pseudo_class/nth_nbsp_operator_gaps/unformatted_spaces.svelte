<style>
	/* U+00A0 on either side of the An+B operator is not whitespace to the respacing:
	   the ASCII gaps normalize to one space each and the character stays put */
	:nth-child(2n   +   1) {
		color: red;
	}

	/* one side only */
	:nth-child(2n   +  1) {
		color: blue;
	}
	:nth-child(2n  +   1) {
		color: green;
	}

	/* the same term in a non-nth functional pseudo */
	:is(2n   +   1) {
		color: teal;
	}
</style>
