<script>
	// a blank line between params is preserved (forces expansion), matching prettier
	function f1(
		a,

		b
	) {}

	// no blank line stays inline
	function f2(a, b) {}

	// a blank line before a glued block comment is preserved - the comment leads its param,
	// so the blank line sits between the previous param's trailing comment and the comment
	function f3(
		a, // x

		/* c */ b
	) {}

	// a JSDoc cast owns its comment the same way
	function f4(
		a, // x

		b = /** @type {T} */ (c)
	) {}
</script>
