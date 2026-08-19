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

	// a curried arrow freezes at the `=>` the directive follows; the outer signature is
	// parent-owned
	const ooo = (ppp) => (qqq) =>
		// prettier-ignore
		rrr  +  sss;

	// a sibling arrow the freeze does not reach still normalizes
	const ttt = () => uuu + vvv;
</script>
