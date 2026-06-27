<script>
	async function fn1() {
		// trailing block comment after operand
		await x; /* c */

		// trailing line comment after operand
		await x; // c

		// trailing own-line block comment
		await x;
		/* c */
	}

	function* fn2() {
		// trailing block comment after operand
		yield x; /* c */

		// trailing line comment after operand
		yield x; // c

		// trailing own-line block comment
		yield x;
		/* c */
	}
</script>
