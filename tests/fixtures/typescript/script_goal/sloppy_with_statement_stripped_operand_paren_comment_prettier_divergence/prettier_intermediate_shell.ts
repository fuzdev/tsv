// a comment written last inside a redundant pair down the header expression's right edge
// stays in the header, after the expression, as it does without the pair
with (
	a +
		b // c1
) {
}

with (
	a &&
		b // c2
) {
}

with (
	a +
		(b +
			c) // c3
) {
}

with (
	a +
		b
		// c4
) {
}

with (
	a +
		b
		/* c5 */
) {
}

// a multi-line block takes the same place
with (
	a +
		b /* c6
		 */
) {
}
