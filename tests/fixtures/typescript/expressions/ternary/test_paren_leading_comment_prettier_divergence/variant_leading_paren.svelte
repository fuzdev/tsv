<script>
	// A leading block comment glued inside a ternary operand's grouping parens
	// (test position and both branches) is preserved where the author wrote it,
	// inside the parens. prettier relocates it out across the paren boundary
	// (`/* c */ (x ?? y)`); the leading-paren form is dual-stable — see README.
	const a = /* c */ (x ?? y) ? p : q;
	const b = cond ? /* c */ (x ?? y) : q;
	const c = cond ? p : /* c */ (x ?? y);

	// a nested ternary TEST keeps the pair for its own reason — `?:` is right-associative, so
	// bare the test re-binds into the enclosing ternary's alternate — and the comment stays
	// inside it
	const e = /* c */ (f ? g : h) ? p : q;

	// every operand at once
	const d = /* c1 */ (x ?? y) ? /* c2 */ (p ?? q) : /* c3 */ (r ?? s);
</script>
