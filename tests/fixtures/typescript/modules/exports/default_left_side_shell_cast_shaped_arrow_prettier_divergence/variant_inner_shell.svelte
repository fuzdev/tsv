<script>
	// JSDoc cast text glued to an arrow's parameter `(` is no cast — the parser reads that
	// `(` as the parameter list's — so inside the exported value's leftmost shell it is an
	// ordinary block that ends its line, and the value hangs under `export default`. Its
	// output wraps the arrow in a pair, which makes a real cast the author gave a line of
	// its own, and that hangs too.
	export default /** @type {T} */ ((x) => zz) + q;
</script>
