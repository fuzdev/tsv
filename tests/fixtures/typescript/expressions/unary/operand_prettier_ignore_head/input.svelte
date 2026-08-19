<script lang="ts">
	// An own-line directive in a unary operator's →operand gap freezes the whole operand.
	// The comment-holder parens are the printer's and stay outside the frozen slice.
	const aaa = typeof (
		// prettier-ignore
		bbb  ??  ccc
	);

	const ddd = !(
		// prettier-ignore
		eee  ??  fff
	);

	const ggg = -(
		// prettier-ignore
		hhh  +  iii
	);

	const jjj = void (
		// prettier-ignore
		kkk  +  lll
	);

	// An operand the author left bare freezes the same way — the shell is re-added to
	// hold the comment either way.
	fn1(
		typeof (
			// prettier-ignore
			mmm.  nnn
		)
	);

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	const ppp = !(
		/* prettier-ignore */
		qqq  ??  rrr
	);

	// The slice→`)` gap belongs to the shell, not to the slice: a block stays inline
	// behind the frozen operand.
	const sss = !(
		// prettier-ignore
		ttt  ??  uuu /* c */
	);

	// A `//` in that gap opens the pair around it — inline it would swallow the `)`.
	const vvv = !(
		// prettier-ignore
		www  ??  xxx // c
	);
</script>
