<script>
	// a block comment after the comma leads the next param - it stays there
	function fn1(a, /* c */
b) {}

	// contrast - a comment before the comma trails the previous param
	function fn2(a /* c */, b) {}

	// a comment given a line of its own expands the param list
	function fn3(
		a,
		/* c */
		b
	) {}
</script>
