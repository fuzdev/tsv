<script>
	// A line comment authored on a property's `:` line trails the `:`; the value hangs
	// one level in below it.
	const a = {
		// c1
		k: b,
		j: 1
	};

	// A run keeps its first comment on the `:` line and the rest on their own lines, in
	// order, with an author blank before the value.
	const c = {
		// c2
		k:
			// c3

			b
	};

	// A block ahead of the line comment on the `:` line stays there with it.
	const d = {
		/* c4 */ // c5
		k: b
	};

	// Computed and quoted keys take the same gap.
	const e = {
		// c6
		[k]: b,
		// c7
		'a-b': b
	};

	// An object value and a function value hang below the comment too.
	const f = {
		// c8
		k: { a: 1 },
		// c9
		m: () => 1
	};

	// A property of an object passed as an argument.
	fn({
		// c10
		k: b
	});
</script>
