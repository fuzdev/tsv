<script lang="ts">
	// a comment written last inside a redundant pair down the header expression's right edge
	// stays in the header, after the expression, as it does without the pair
	if (
		a + b // c1
	) {
	}

	if (
		a &&
		b // c2
	) {
	}

	if (
		a +
		(b + c) // c3
	) {
	}

	if (
		a + b
		// c4
	) {
	}

	if (
		a + b
		/* c5 */
	) {
	}

	if (
		a + b /* c7 */ // c7
	) {
	}

	if (
		a + b // c8
	) /* d8 */ {
	}

	if (
		a.b + c // c9
	)
		x();

	// a multi-line block takes the same place
	if (
		a + b /* c6
		 */
	) {
	}
</script>
