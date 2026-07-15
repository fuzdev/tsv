<script>
	switch (a) {
		// A block glued to the first case leads it.
		/* block */ case 1:
			break;
		// A block glued to the next case leads it, even when a line comment shares the gap.
		case 2: // line
		/* block */ case 3:
			break;
		// Blocks glued to each other but not to the case stay together on their own line.
		case 4:
			break;
		/* block1 */ /* block2 */
		case 5:
			break;
	}
</script>
