<script>
	// A trailing elision's comma is syntactically significant - it survives
	const [a, ,] = arr;

	// Two trailing elisions
	const [b, , ,] = arr;

	// Only elisions
	const [, ,] = arr;

	// Nested
	const [[c, ,]] = arr;

	// In a parameter list
	function fn([d, ,]) {
		return d;
	}

	// As an assignment target
	let e;
	[e, ,] = arr;
</script>
