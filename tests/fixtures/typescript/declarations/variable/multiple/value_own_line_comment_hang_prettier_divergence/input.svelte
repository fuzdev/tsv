<script>
	// An own-line comment in a multi-declarator's value gap hangs the value — the `=` ends
	// its line — whether the gap emits the comment or the value OWNS it.
	const cast =
		/** @type {A} */
		(expr),
		a = 1;

	const block =
		/**
		 * c
		 */ 1,
		b = 2;

	// The gap-emitted spelling, unchanged: the anchor the two owned ones now match.
	const gap =
		// c
		1,
		c = 3;
</script>
