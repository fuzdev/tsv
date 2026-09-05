<script>
	// An own-line comment in a multi-declarator's value gap hangs the value — the `=` ends
	// its line — whether the gap emits the comment or the value OWNS it. The hung value
	// sits one level past the declarator, two past the keyword.
	const cast =
			/** @type {A} */
			(expr),
		a = 1;

	const block =
			/**
			 * c
			 */ 1,
		b = 2;

	// The gap-emitted spelling.
	const gap =
			// c
			1,
		c = 3;
</script>
