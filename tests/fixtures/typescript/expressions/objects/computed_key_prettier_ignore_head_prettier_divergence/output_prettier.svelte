<script lang="ts">
	// An own-line directive in a computed KEY's `[`→key gap freezes the whole key. The
	// brackets belong to the property, so they stay outside the slice.
	const aaa = {
		[// prettier-ignore
		bbb  +  ccc]: ddd
	};

	class Eee {
		[// prettier-ignore
		fff  +  ggg] = hhh;

		[// prettier-ignore
		iii  +  jjj]() {}
	}

	// A destructuring pattern's computed key is the same gap one host over.
	const {
		[// prettier-ignore
		kkk  +  lll]: mmm
	} = nnn;

	// A type-literal member's computed key reaches the same bracket emitter — but takes NO
	// clarity parens, where the value spellings above print `[(ppp = qqq)]`. The frozen slice
	// gets the shell its OWN position's builder supplies, not its neighbours'.
	type Aaa1 = {
		[// prettier-ignore
		bbb1  +  ccc1]: ddd1;
		[// prettier-ignore
		eee1  =  0]: fff1;
	};

	// Parens the key REQUIRES are the printer's, not the author's, so they ride outside
	// the frozen slice.
	const ooo = {
		[// prettier-ignore
		(ppp  =  qqq)]: rrr
	};

	// An own-line block comment behaves identically — placement keys the freeze, not the
	// spelling.
	const sss = {
		[/* prettier-ignore */
		ttt  +  uuu]: vvv
	};

	// The key→`]` gap belongs to the brackets, not to the slice.
	const www = {
		/* c */
		[// prettier-ignore
		xxx  +  yyy]: zzz
	};
</script>
