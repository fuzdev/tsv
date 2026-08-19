<script lang="ts">
	// An own-line directive in a spread's `...`→argument gap freezes the whole argument.
	// The directive keeps the line the author gave it, so the argument hangs below it.
	fn1(
		...// prettier-ignore
		(aaa  ??  bbb)
	);

	const ccc = [
		...// prettier-ignore
		(ddd  ??  eee)
	];

	const fff = {
		...// prettier-ignore
		(ggg  ??  hhh)
	};

	// An argument needing no parens freezes bare.
	fn1(
		...// prettier-ignore
		iii.  jjj
	);

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	fn1(
		.../* prettier-ignore */
		(kkk  ??  lll)
	);

	// The slice→`)` gap belongs to the shell, not to the slice.
	fn1(
		...// prettier-ignore
		(mmm  ??  nnn) /* c */
	);

	// A SEQUENCE argument's parens are its own node's, not the context's, so the frozen
	// slice gets them back rather than losing the grouping.
	fn1(
		...// prettier-ignore
		(qqq, rrr)
	);

	// A directive the author glued to the `...` is INERT under the placement floor: the
	// comment keeps the line it was written on and the argument normalizes.
	fn1(.../* prettier-ignore */ (ooo ?? ppp));
</script>
