<script>
	// Nested array destructuring (inner array converts to a pattern deeply)
	for ([a, [b, c]] of arr) {
		expr;
	}

	// Nested object destructuring (inner object converts to a pattern deeply)
	for ([a, { b }] of arr) {
		expr;
	}

	// Valid rest as the last array element (rest stays accepted)
	for ([a, ...rest] of arr) {
		expr;
	}

	// Valid rest as the last object property, for-in branch
	for ({ a, ...rest } in obj) {
		expr;
	}
</script>
