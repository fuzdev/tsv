<script lang="ts">
	// a comment written last inside a redundant pair down the header expression's right edge
	// stays in the header, after the expression, as it does without the pair
	do {} while (a + (b // c1
	));

	do {} while (a && (b // c2
	));

	do {} while (a + (b + (c // c3
	)));

	do {} while (a + (b
	// c4
	));

	do {} while (a + (b
	/* c5 */
	));

	// a multi-line block takes the same place
	do {} while (a + (b /* c6
	 */));
</script>
