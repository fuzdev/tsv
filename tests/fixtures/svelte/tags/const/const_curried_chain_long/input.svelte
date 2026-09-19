{#if cond}
	<!-- A curried chain that fits stays on the `=` line -->
	{@const a = (x) => (y) => x}

	<!-- A chain too long for the line breaks after the `=` and stacks its heads at one indent -->
	{@const b =
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) =>
			ddddddddd}

	<!-- A block body keeps the last head's line -->
	{@const c =
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) =>
		(bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		(cccccccccccccccccccccccccccccccccc) => {
			return d;
		}}

	<!-- A head that forces the break (a destructured parameter) breaks however short the chain is -->
	{@const d =
		({ x }) =>
		(y) =>
			x}

	<!-- 100 columns: fits -->
	{@const e = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => cccccc}

	<!-- 101 columns: the body drops below the heads -->
	{@const f = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) =>
		ccccccc}

	<p>{a}{b}{c}{d}{e}{f}</p>
{/if}
