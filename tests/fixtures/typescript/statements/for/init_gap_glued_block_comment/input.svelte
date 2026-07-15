<script>
	// A block glued to the next declarator leads it, even when a line comment shares the gap.
	for (
		let i1 = 0, // line
			/* block */ j1 = 1;
		i1 < j1;
		i1++
	) {}

	// Blocks glued to each other but not to the declarator stay together on their own line.
	for (
		let indexAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa = 0,
			/* block1 */ /* block2 */
			j2 = 1;
		indexAaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa < j2;
		j2++
	) {}
</script>
