<script lang="ts">
	// a comment written last inside a redundant pair down the header expression's right edge
	// stays in the header, after the expression, as it does without the pair
	switch (a + (b // c1
	)) {}

	switch (a && (b // c2
	)) {}

	switch (a + (b + (c // c3
	))) {}

	switch (a + (b
	// c4
	)) {}

	switch (a + (b
	/* c5 */
	)) {}

	// a multi-line block takes the same place
	switch (a + (b /* c6
	 */)) {}
</script>
