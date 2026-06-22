<script>
	// Nested call with array arg (depth exhausts at array elements)
	a.fn1(b.fn2([x, y]))
		.fn3()
		.fn4();

	// Arrow function is always non-simple
	a.fn1((x) => x.y)
		.fn2()
		.fn3();

	// Simple args stay inline even with 3+ calls
	a.fn1('x').fn2('y').fn3('z');

	// Object with simple nested call stays inline (0-arg call is simple)
	a.fn1({ key: nested.call() }).fn2().fn3();
</script>
