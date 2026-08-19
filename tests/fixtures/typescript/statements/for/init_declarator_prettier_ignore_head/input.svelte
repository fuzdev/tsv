<script lang="ts">
	// an own-line directive in an init declarator's `=`→value gap freezes the whole value;
	// the header's `;` stays parent-owned
	for (
		let aaa =
			// prettier-ignore
			bbb  +  ccc;
		aaa < 10;
		aaa++
	) {
		fn();
	}

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	for (
		let ddd =
			/* prettier-ignore */
			eee  +  fff;
		ddd < 10;
		ddd++
	) {
		fn();
	}

	// a sibling declarator the freeze does not reach still normalizes
	for (
		let ggg =
				// prettier-ignore
				hhh  +  iii,
			jjj = kkk + lll;
		ggg < 10;
		ggg++
	) {
		fn();
	}

	// the declarator position's clarity parens are the printer's, so they stay OUTSIDE the
	// frozen slice
	for (
		let mmm =
			// prettier-ignore
			(nnn  =  ooo);
		mmm < 10;
		mmm++
	) {
		fn();
	}

	// the header's `[~In]` parens are the printer's too
	for (
		let ppp =
			// prettier-ignore
			(qqq  in  rrr);
		ppp;
		ppp
	) {
		fn();
	}

	// a multi-line frozen value keeps its verbatim layout
	for (
		let sss =
			// prettier-ignore
			{
					ttt:   1,
				uuu: 2
			};
		sss;
		sss
	) {
		fn();
	}

	// a paren the frozen value does not need strips, and a block comment inside it stays
	// inline — the clause separator is not a terminator it may defer past
	for (
		let vvv =
			// prettier-ignore
			www  +  xxx /* c */;
		vvv;
		vvv
	) {
		fn();
	}
</script>
