<script lang="ts">
	// a comment written last inside a redundant pair down the header expression's right edge
	// stays in the header, after the expression, as it does without the pair
	while (a + (b // c1
	)) {}

	while (a && (b // c2
	)) {}

	while (a + (b + (c // c3
	))) {}

	while (a + (b
	// c4
	)) {}

	while (a + (b
	/* c5 */
	)) {}

	// a multi-line block takes the same place
	while (a + (b /* c6
	 */)) {}
</script>
