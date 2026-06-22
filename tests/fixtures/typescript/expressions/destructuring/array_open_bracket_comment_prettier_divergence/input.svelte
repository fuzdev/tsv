<script>
	// line comment trailing `[` - single element
	const [ // c1
		w
	] = a1;

	// line comment trailing `[` - multiple elements
	const [ // c2
		x,
		y
	] = a2;

	// block comment trailing `[`, then own-line leading comment
	const [ /* bracket */
		// lead
		z
	] = a3;
</script>
