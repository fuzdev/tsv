<script>
	// String value
	const a = {
		property_name: 'https://fuz.dev/path/to/some/resource/value' // This is a trailing comment that describes the property in detail
	};

	// Call expression value
	const b = {
		prop: fn(a, b, c) // trailing comment that makes this exceed the print width limit of one hundred
	};

	// Array value
	const c = {
		prop: [1, 2, 3] // trailing comment that makes this exceed the print width limit of one hundred chars
	};

	// Object value
	const d = {
		prop: { a: 1 } // trailing comment that makes this exceed the print width limit of one hundred chars
	};
</script>
