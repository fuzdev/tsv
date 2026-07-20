<script>
	function* g() {
		// glued single-line block — collapses inline, no parens needed
		yield   /* c */   x ?? y;

		// own-line block — parens retained, so the author's break stays legal
		// (`yield` is a restricted production: a newline after it triggers ASI)
		yield (
/* c */
x ?? y);

		// line comment — always forces the break, so always retains the parens
		yield (
// c
x ?? y);

		// same in value position
		const a = yield (
/* c */
x ?? y);

		// the delegate form is restricted too
		const b = yield* (
// c
gen());
	}
</script>
