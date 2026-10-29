<script lang="ts">
	// A block comment written last inside a binary chain's stripped operand shell, at the
	// end of an expression statement or a declaration's last declarator, trails the
	// statement past its `;` in one pass.
	x + (y /* c */);

	v = a + (b /* c */);

	let w = a + (b /* c */);

	let p = 1, r = a + (b /* c */);

	// A non-last declarator's comment stays before its comma.
	let s = a + (b /* c */), t = 1;

	// A pair the operator requires stays in the output, and a comment written last inside
	// it still trails the statement.
	x = a + (q + b /* c */);

	x = a || (b && c /* c */);

	x = a + (b as T /* c */);

	// The pair a declarator position adds around an assignment value holds a block that
	// ends a binary inside it.
	const k = x = y + (b /* c */);

	var v = y += a + (b /* c */);

	// The value positions the statement's `;` follows answer the same.
	x = c ? d : a + (b /* c */);

	x = () => a + (b /* c */);

	// So does the pair an arrow adds around its conditional body.
	x = () => c ? d : a + (b /* c */);

	x = c ? d : (aaa ? bbb : ccc /* c */) /* d */;

	if (a) x = a + (b /* c */);

	if (b) y = (b /* m
	n */);

	if (c) y = (b /* c */) /* d */;
	else z();

	// A multi-line block trails the same way, and the chain keeps its flat layout.
	x = a + (b /* m
	n */);

	// A comment after the shell keeps its place behind the one inside it.
	x = a + (b /* c */) /* d */;

	y = (b /* c */) /* d */;

	x = a + (b /* c */) /* d */; // e

	// A value a format-ignore directive freezes prints its region verbatim, once.
	x =
		// prettier-ignore
		a  +  (b /* c */);
</script>
