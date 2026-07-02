<style>
	/* comment before the An+B */
	li:nth-child(/* c1 */ 2n) {
		color: red;
	}

	/* comment after the An+B (part of the expression text, kept verbatim) */
	li:nth-child(2n /* c2 */) {
		color: blue;
	}

	/* a comment freezes the An+B text verbatim - no operator respacing inside it */
	li:nth-child(2n + 1 /* a-b */) {
		color: green;
	}

	/* comment between the An+B and the of keyword (kept verbatim) */
	li:nth-child(3 /* c3 */ of .class1) {
		color: teal;
	}

	/* consecutive comments between the of keyword and the selector list */
	li:nth-child(3 of /* c4 */ /* c5 */ .class2) {
		color: purple;
	}

	/* comment after the last selector of the of list */
	li:nth-child(3 of .class3, .class4 /* c6 */) {
		color: orange;
	}
</style>
