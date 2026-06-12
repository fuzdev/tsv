{#if a}
	{@const {b, c} = expr}
	{@const [b, c] = expr}
	{@const {b = 1} = expr}
	{@const {
		b: {c},
	} = expr}
	{@const {
		a: [b, c = 1],
		d: {e, ...rest},
		f: g = 2,
	} = expr}
	{b}
{/if}
