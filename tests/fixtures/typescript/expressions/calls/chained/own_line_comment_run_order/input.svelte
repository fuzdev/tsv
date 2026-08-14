<script>
	// An own-line comment run in a chain gap keeps its authored order: line and
	// block comments interleave as written, never regrouped by kind.
	obj
		// c1
		/* c2 */
		.method1();

	// The same run between two chain segments.
	obj
		.method1()
		// c3
		/* c4 */
		.method2();

	// The mirror authoring, block comment first.
	obj
		/* c5 */
		// c6
		.method1();

	// A longer interleave across a multi-segment call chain.
	fn()
		// c7
		/* c8 */
		// c9
		.method1()
		.method2();
</script>
