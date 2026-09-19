<script>
	function* gen() {
		// a block comment between `yield` and an assignment operand stays in front of the
		// operand's clarity parens
		yield /* c */ (a = b);
		const c = yield /* c */ (d ??= e);

		// null control: the operand the comment leads needs no parens, so none print
		yield /* c */ f ?? g;
	}
</script>
