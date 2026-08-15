<script>
	// A multiline block comment's own interior blank is not an author blank between
	// two comments - the `}`->`else` gap measures from the `*/`, not from the `}`
	if (a) {
	} /* c1

	*/
	/* c2 */
	else {
	}

	// The same with a line comment below it
	if (b) {
	} /* c1

	*/
	// c2
	else {
	}

	// Null control - the same run with no interior blank
	if (c) {
	} /* c1
	 */
	/* c2 */
	else {
	}
</script>
