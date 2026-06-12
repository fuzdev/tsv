<script>
	function fn1() {
		// trailing block comment after operand
		return x /* c */;

		// trailing line comment after operand
		return x; // c

		// trailing own-line block comment
		return x;
		/* c */
	}

	function fn2() {
		// trailing block comment after operand
		throw x /* c */;
	}
</script>
