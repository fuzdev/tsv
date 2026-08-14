<script>
	// An author blank between two comments in the run separates two remarks and
	// survives.
	fn()
		// c1

		/* c2 */
		.method1();

	// A pair the author glued stays glued (the block leads the line comment
	// inline); the run still ends the line.
	fn()
		// c3
		/* c4 */ // c5
		.method1();

	// A block glued to the member hugs it across the break.
	fn()
		/* c6 */ .method1();

	// An author blank between the run's last comment and the member survives.
	fn()
		// c7

		.method1();

	// An own-line block keeps its own line (control case).
	fn()
		/* c8 */
		.method1();
</script>
