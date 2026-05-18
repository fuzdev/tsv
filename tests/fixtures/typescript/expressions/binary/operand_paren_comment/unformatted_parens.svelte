<script>
	// LHS leading block comment in stripped parens
	const a1 = (/* c */ x) + y;
	const a2 = (/* c */ x) && y;

	// LHS trailing block comment in stripped parens
	const b1 = (x /* c */) + y;
	const b2 = (x /* c */) && y;

	// RHS leading block comment in stripped parens
	const c1 = x + (/* c */ y);
	const c2 = x && (/* c */ y);

	// LHS leading line comment in stripped parens (multiline)
	const d1 =
		(
			// c
			x
		) + y;

	// Statement-level: LHS leading block in stripped parens
	(/* c */ x) + y;
</script>
