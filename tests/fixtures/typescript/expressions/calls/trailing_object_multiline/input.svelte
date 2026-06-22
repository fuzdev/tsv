<script>
	// Call expression arg + multiline object - hugs args with opening brace
	fn(call(a), {
		prop1: 1,
		prop2: 2
	});

	// Identifier arg + multiline object (existing working case for contrast)
	fn(arg, {
		prop1: 1,
		prop2: 2
	});

	// Member expression arg + multiline object - should also hug
	fn(obj.prop, {
		prop1: 1,
		prop2: 2
	});

	// Multiple args with call in middle + trailing object
	fn(arg1, call(b), {
		prop1: 1
	});

	// New expression arg + multiline object
	fn(new Cls(a), {
		prop1: 1,
		prop2: 2
	});

	// Chained call arg + multiline object
	fn(a.b().c(), {
		prop1: 1
	});
</script>
