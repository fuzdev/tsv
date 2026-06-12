<script>
	// block body with multiline return object - trailing comma required
	fn(() => {
		return {
			a: 1,
			b: 2,
		};
	});

	// short inline return object (contrast - no comma needed)
	fn(() => {
		return {a: 1};
	});

	// chain context works (contrast)
	items
		.map(() => {
			return {
				a: 1,
			};
		})
		.filter(Boolean);

	// non-arrow function works (contrast)
	fn(function () {
		return {
			a: 1,
		};
	});

	// method call with multiline return object (same bug)
	obj.method(() => {
		return {
			a: 1,
		};
	});
</script>
