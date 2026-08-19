<script lang="ts">
	// An own-line directive in a template interpolation's `${`→expression gap freezes the
	// whole expression. The `${` and `}` are the literal's own and stay outside the slice.
	const aaa = `x${
		// prettier-ignore
		bbb  +  ccc
	}y`;

	// Parens the expression REQUIRES are the printer's, not the author's, so they ride
	// outside the frozen slice.
	const ddd = `x${
		// prettier-ignore
		(eee  =  fff)
	}y`;

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	const ggg = `x${
		/* prettier-ignore */
		hhh  +  iii
	}y`;

	// An honored directive owns its line on BOTH sides: a single-line block the author
	// wrote above it keeps its own line instead of welding onto the directive's.
	const jjj = `x${
		/* c */
		// prettier-ignore
		kkk  +  lll
	}y`;

	// The expression→`}` gap belongs to the literal, not to the slice.
	const mmm = `x${
		// prettier-ignore
		nnn  +  ooo /* c */
	}y`;
</script>
