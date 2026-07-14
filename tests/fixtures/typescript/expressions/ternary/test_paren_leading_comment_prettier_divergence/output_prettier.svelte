<script>
	// A leading block comment glued inside a ternary operand's grouping parens
	// (test position and both branches) is preserved where the author wrote it,
	// inside the parens. prettier relocates it out across the paren boundary
	// (`/* c */ (x ?? y)`); the leading-paren form is dual-stable — see README.
	const a = /* c */ (x ?? y) ? p : q;
	const b = cond ? /* c */ (x ?? y) : q;
	const c = cond ? p : /* c */ (x ?? y);

	// every operand at once
	const d = /* c1 */ (x ?? y) ? /* c2 */ (p ?? q) : /* c3 */ (r ?? s);
</script>
