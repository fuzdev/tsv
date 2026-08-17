<script>
	// line comment trailing `[` - single element
	const [
		// c1
		w
	] = a1;

	// line comment trailing `[` - multiple elements
	const [
		// c2
		x,
		y
	] = a2;

	// block comment trailing `[`, then own-line leading comment
	const [
		/* bracket */
		// lead
		z
	] = a3;

	// an author blank BELOW the pulled comment survives — the blank is authorship, not
	// the container's leading gap (the blank ABOVE one stays erased, against the delimiter)
	const [
		// c3

		v
	] = a4;
</script>
