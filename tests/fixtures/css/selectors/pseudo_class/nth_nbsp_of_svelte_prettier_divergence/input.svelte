<style>
	/* U+00A0 after the `of` keyword is the separator before S; it stays, flush against S */
	:nth-child(2n + 1 of .class1) {
		color: red;
	}

	/* ...and before it, flush against the An+B term, with the keyword's own space after it */
	:nth-child(2n + 1  of .class2) {
		color: blue;
	}

	/* after S, before the `)`, behind the space that closes the last name */
	:nth-child(2n + 1 of .class3  ) {
		color: green;
	}

	/* the same fold in a non-nth functional pseudo, where ` of ` stays inside the Nth value */
	:is(2n + 1 of .class4) {
		color: teal;
	}
</style>
