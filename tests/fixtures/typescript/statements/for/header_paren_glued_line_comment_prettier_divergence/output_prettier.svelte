<script>
	// C-style for - a line comment the author glued to the header `(` keeps that
	// line, and the clauses take their own lines below it
	for (
		// c1
		let i = 0;
		i < n;
		i++
	) {
		b();
	}

	// For-of - same
	for (// c2
	const x of arr) {
		b();
	}

	// For-in - same
	for (// c3
	const k in obj) {
		b();
	}

	// An author blank below the glued comment survives it
	for (
		// c4

		let j = 0;
		j < n;
		j++
	) {
		b();
	}

	// Control - written on its own line the comment keeps its own line
	for (
		// c5
		let m = 0;
		m < n;
		m++
	) {
		b();
	}
</script>
