<script>
	// Blocks glued to each other lead the first property together.
	const a = {
		/* c1 */ /* c2 */
		x: 1
	};

	// Blocks glued to each other lead a later property together.
	const b = {
		x: 1,
		/* c1 */ /* c2 */
		y: 2
	};

	// Three glued blocks stay on one line.
	const c = {
		x: 1,
		/* c1 */ /* c2 */ /* c3 */
		y: 2
	};

	// Blocks on their own lines keep them (control).
	const d = {
		x: 1,
		/* c1 */
		/* c2 */
		y: 2
	};
</script>
