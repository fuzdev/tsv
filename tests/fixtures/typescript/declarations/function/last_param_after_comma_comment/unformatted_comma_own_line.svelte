<script>
	// a block comment after the last param's comma trails the param - the comma is never
	// re-emitted, so the comment keeps the line it shares with it and the list stays inline
	function fn1(a
	, /* c */) {}

	// the same with two params
	function fn2(a, b
	, /* c */) {}

	// an arrow, a class method and an object method take the same rule
	const fn3 = (a
	, /* c */) => {};

	class A {
		fn4(a
		, /* c */) {}
	}

	const obj = {
		fn5(a
		, /* c */) {}
	};

	// a multiline block comment trails the param and expands the list
	function fn6(a
	, /* c1
	   c2 */) {}

	// contrast - a comment the author gave a line of its own keeps it and expands the list
	function fn7(a,
	/* c */) {}
</script>
