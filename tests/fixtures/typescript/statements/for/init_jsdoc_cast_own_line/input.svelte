<script>
	// An own-line JSDoc cast in the init's value gap hangs after `=`, like a
	// statement declarator's.
	for (
		let a =
			/** @type {A} */
			(b);
		a < 9;
		a++
	) {}

	// An inline cast stays on the `=` line and keeps the header flat.
	for (let c = /** @type {A} */ (d); c < 9; c++) {}
</script>
