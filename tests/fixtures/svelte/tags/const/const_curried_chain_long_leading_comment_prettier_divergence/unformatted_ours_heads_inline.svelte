{#if cond}
	<!-- No comment (the null control): the chain breaks after the `=`, its heads at one indent -->
	{@const a = (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}

	<!-- A line comment on the `=` line keeps that line; the heads keep their one indent -->
	{@const b = // c
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}

	<!-- A single-line block glued to the first head leads it, below the `=` -->
	{@const c = /* c */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}

	<!-- An indentable block glued to the first head leads it the same way -->
	{@const d =
		/**
		 * c
		 */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}

	<!-- A preserved multi-line block leads the chain from the `=` line -->
	{@const e = /* c
		d */ (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}

	<!-- A comment on a line of its own above the chain: the heads past the first indent under it -->
	{@const f =
		// c
		(aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}

	<p>{a}{b}{c}{d}{e}{f}</p>
{/if}
