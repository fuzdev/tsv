<script>
	// A `//` written inside the value's own grouping parens keeps those parens, rather
	// than deferring past the `;` onto a line that may already hold one — two `//`s
	// sharing an output line merge into a single comment.
	x; // c1
	// c2

	// the parens the statement would print anyway are the same single pair
	({ a: 1 }); // c

	(function () {}); // c

	// a comment past the closing paren is outside the parens, so it trails the `;`
	x; // c
	/* t */

	// Control: a trailing block needs no shell — it does not end its line, so its
	// placement past the `;` is lossless and stable in both formatters.
	x; /* c */
</script>
