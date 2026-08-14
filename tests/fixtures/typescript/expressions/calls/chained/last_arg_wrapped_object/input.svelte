<script>
	// The last call's single argument WRAPS its object (a `new` expression), so the
	// chain stays flat and the call's own arguments expand, the object breaking
	// inside the wrapper — the wrapper is not itself an object, so it does not hug.
	const a1 = expr.fn1().fn2(
		new A(b, {
			prop1: 'value1value1value1value1',
			prop2: 'value2value2value2value2',
			prop3: 'value3value3value3value3'
		})
	);

	// Same shape with a plain call wrapper.
	const a2 = expr.fn1().fn2(
		fn3({
			prop1: 'value1value1value1value1',
			prop2: 'value2value2value2value2',
			prop3: 'value3value3value3value3'
		})
	);
</script>
