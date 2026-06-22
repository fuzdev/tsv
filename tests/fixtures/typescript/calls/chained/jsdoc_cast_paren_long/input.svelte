<script>
	// Short: nested JSDoc casts inline in a chain call arg
	set = a.b(
		c.d([e.f('g')], /** @type {T} */ (h.visit(i.k('=', /** @type {U} */ (j.m), e.f('g')))))
	);

	// Long: JSDoc cast wrapping a call that expands inside the preserved parens
	a.b(
		[c],
		/** @type {AAAA} */ (
			d.eee(fffffffffffffffffffffffffffffffffffffffffffff, gggggggggggggggggggggggg)
		)
	);
</script>
