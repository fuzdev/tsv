<script>
	// inline-adjacent block comment between params stays inline
	function fn1(a, /* c */ b) {}

	// line comment before a param expands the list (a line comment cannot be inline)
	function fn2(
		a,
		// d
		b
	) {}

	// block comment on its own line before a param expands the list
	function fn3(
		a,
		/* e */
		b
	) {}

	// blank lines around an own-line comment are preserved
	function fn4(
		a,

		/* f */

		b
	) {}

	// block comment trailing a param before its comma stays inline
	function fn5(a /* g */, b) {}
</script>
