<script>
	function* g() {
		// glued single-line block — collapses inline, no parens needed
		yield /* c */ x ?? y;

		// own-line block — parens retained, so the author's break stays legal
		// (`yield` is a restricted production: a newline after it triggers ASI)
		yield /* c */
		x ?? y;

		// a glued block that SPANS a line forces the break too — a MultiLineComment holding a
		// line terminator IS one for ASI (ECMA-262 §12.4) — so the parens are retained even
		// though the comment is glued to the operand
		yield /* a
b */ x ?? y;

		// line comment — always forces the break, so always retains the parens
		yield // c
		x ?? y;

		// same in value position
		const a = yield /* c */
		x ?? y;

		// the same, in value position
		const c = yield /* a
b */ x ?? y;

		// the delegate form takes the same pair, one rule for all three restricted
		// productions — a bare `yield*` is a syntax error, so ASI cannot split it and the
		// divergence there is layout alone
		yield* /* a
b */ gen();

		// the delegate form is restricted too
		const b =
			yield* // c
			gen();
	}
</script>
