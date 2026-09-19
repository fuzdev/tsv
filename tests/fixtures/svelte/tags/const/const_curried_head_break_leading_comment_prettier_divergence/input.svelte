{#if cond}
	<!-- No comment (the null control): heads that force the break stack under the `=` -->
	{@const a =
		({}) =>
		() =>
			b}

	<!-- A line comment on the `=` line keeps that line; the heads keep their one indent -->
	{@const c = // c
		({}) =>
		() =>
			b}

	<!-- A single-line block glued to the first head leads it, below the `=` -->
	{@const d =
		/* c */ ({}) =>
		() =>
			b}

	<!-- A run of blocks leads it the same way -->
	{@const e =
		/* c1 */ /* c2 */ ({}) =>
		() =>
			b}

	<!-- An indentable block glued to the first head leads it the same way -->
	{@const f =
		/**
		 * c
		 */ ({}) =>
		() =>
			b}

	<!-- A preserved multi-line block leads the chain from the `=` line -->
	{@const g = /* c
d */ ({}) =>
		() =>
			b}

	<p>{a}{c}{d}{e}{f}{g}</p>
{/if}
