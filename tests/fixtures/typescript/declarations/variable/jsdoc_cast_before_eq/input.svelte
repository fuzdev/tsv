<script>
	// Block comment before = is not a cast (no paren follows); RHS cast is preserved
	var a /** @type {number} */ = /** @type {string} */ (b.c);

	// Nested JSDoc casts with a long call expression
	const d /** @type {any[]} */ = /** @type {A} */ (
		e.f.find((x) => x.type === 'aaaa' && x.name === 'bbbbb' && x.value !== true)
	);
</script>
