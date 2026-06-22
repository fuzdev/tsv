<script>
	const a = 1,
		b = 2,
		c = 3,
		d = 4,
		e = 5;

	// single specifier with trailing line comment
	export {
		a // comment
	};

	// last specifier trailing line comment
	export {
		b,
		c // comment
	};

	// all specifiers with trailing line comments
	export {
		d, // comment1
		e // comment2
	};
</script>
