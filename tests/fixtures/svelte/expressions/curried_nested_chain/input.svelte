<!-- A curried chain nested in another chain resolves on its own in template expressions -->
<p>
	{(
		(a) => (b) =>
			fn((c) => (d) => e)
	)()}
</p>
<p>
	{((a) => (b) => ({
		k: fn(
			(x = 1) =>
				(y) =>
					z
		)
	}))()}
</p>
<p>
	{fn(
		(a = (x) => (y) => z) =>
			(b) =>
				c
	)}
</p>
<Comp
	prop={fn(
		(a = (x) => (y) => z) =>
			(b) =>
				c
	)}
/>

{#if cond}
	{@const f = fn(
		(a = (x) => (y) => z) =>
			(b) =>
				c
	)}
	{@const g = (
		(a) => (b) =>
			fn((c) => (d) => e)
	)()}
	{f}{g}
{/if}
