<script>
	// Two own-line comments before the condition, no authored blank
	do {} while (
		// c1
		// c2
		a
	);

	// Same, with an authored blank between them
	do {} while (
		// c3

		// c4
		a
	);

	// Three own-line comments before the condition, no authored blank
	do {} while (
		// c5
		// c6
		// c7
		a
	);

	// Own-line comments after the condition
	do {} while (
		a
		// c8
		// c9
	);
</script>
