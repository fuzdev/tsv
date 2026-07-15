<script>
	// A block glued to the next declarator leads it, even when a line comment shares the gap.
	let a1 = 1, // line
		/* block */ b1 = 2;

	// Blocks glued to each other but not to the declarator stay together on their own line.
	let a2 = 1,
		/* block1 */ /* block2 */
		b2 = 2;

	// A glued pair after a line comment stays glued.
	let a3 = 1, // line
		/* block1 */ /* block2 */
		b3 = 2;

	// Blocks the author put on separate lines keep them.
	let a4 = 1, // line
		/* block1 */
		/* block2 */
		b4 = 2;
</script>
