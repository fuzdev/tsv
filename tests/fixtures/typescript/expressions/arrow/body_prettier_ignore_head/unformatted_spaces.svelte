<script lang="ts">
	// an own-line directive in the `=>`→body gap freezes the whole body
	const aaa = () =>
		// prettier-ignore
		bbb  +  ccc;

	// an own-line block comment behaves identically — placement keys the freeze, not the spelling
	const ddd = () =>
		/* prettier-ignore */
		eee  +  fff;

	// an object body's parens are the printer's, so they stay OUTSIDE the frozen slice
	const ggg = () =>
		// prettier-ignore
		({ hhh:   1 });

	// a sequence body's required pair is its own and stays outside the slice too
	const iii = () =>
		// prettier-ignore
		(jjj,   kkk);

	// a multi-line frozen body keeps its verbatim layout
	const lll = () =>
		// prettier-ignore
		({
				mmm:   1,
			nnn: 2
		});

	// a COMPOSITE body — one whose leftmost node is an object rather than the body's root —
	// carries the pair INSIDE the slice: acorn's member, call and binary spans begin at the
	// author's `(`, so the freeze takes the pair along with the object and prints it back
	// verbatim — the position-pair seam mints that same form from the shell authoring
	// instead, which the paren-shell variant carries
	const yyy = () =>
		// prettier-ignore
		({zzz:   1}).aaa1;

	const bbb1 = () =>
		// prettier-ignore
		({ccc1:   1})();

	const ddd1 = () =>
		// prettier-ignore
		({eee1:   1}) + 1;

	// a curried arrow freezes at the `=>` the directive follows; the outer signature is
	// parent-owned
	const ooo = (ppp) => (qqq) =>
		// prettier-ignore
		rrr  +  sss;

	// a frozen body that is ITSELF an arrow keeps no freeze: the arrow chain is reprinted
	// arrow by arrow, so the directive stays on the line the author gave it and the body
	// normalizes around it
	const fff1 =
		(ggg1) =>
		(hhh1) =>
		// prettier-ignore
		(iii1) =>
			ggg1  +  hhh1   +   iii1;

	const jjj1 =
		(kkk1) =>
		// prettier-ignore
		(lll1) =>
			kkk1   +   lll1;

	const mmm1 =
		(nnn1) =>
		(ooo1) =>
		// prettier-ignore
		(ppp1) => {
			return   nnn1  +  ooo1;
		};

	// a BLOCK body freezes whole, braces included
	const www = () =>
		// prettier-ignore
		{   xxx  ;   };

	// a labeled statement opens a BLOCK body too — the `{` is the body's, not an object's —
	// and the block freezes with it
	const qqq1 = () =>
		// prettier-ignore
		{rrr1:   1};

	// a sibling arrow the freeze does not reach still normalizes
	const ttt = () => uuu + vvv;
</script>
