<script>
	// An own-line directive in the `=>`→body gap freezes the BODY — the body is the value
	// the gap heads, so the whole expression rides the freeze.
	const a1 = (e) =>
		// prettier-ignore
		e.prop   +   1;

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	const b1 = (e) =>
		/* prettier-ignore */
		e.prop   +   2;

	// A directive the author put on the `=>`'s line is INERT under the placement floor: the
	// comment keeps the line it was written on and the body normalizes.
	const c1 = (e) =>
		// prettier-ignore
		e.prop   +   3;

	const d1 = (e) => /* prettier-ignore */ e.prop   +   4;
</script>
