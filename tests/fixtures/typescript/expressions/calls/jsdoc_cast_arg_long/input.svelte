<script>
	// Short: JSDoc cast wrapping a call arg, fits on one line
	a(b, /** @type {T} */ (fn(c, d, e)));

	// Long: the cast's inner call expands inside the preserved parens
	a(
		b______,
		/** @type {Expression} */ (fn(c, d, e__________________________________________________))
	);

	// Long with a longer preceding arg
	a(
		b________________________________________,
		/** @type {Expression} */ (fn(c, d, e__________________________________________________))
	);
</script>
