<script>
	// the same pair in a `<script>`: the statement's own line end takes the comment
	const q = x + (y * z // c0
	);
</script>

<p>{a || (b || c
// c1
)}</p>

<div data-a={x + (y * z
// c2
)}></div>

{#if x + (y * z
// c3
)}
	<p>t</p>
{/if}

{#each a instanceof (b
// c4
) as item}
	<p>{item}</p>
{/each}

{#if cond}
	{@const r = x + (y * z
	// c5
	)}
	{r}
{/if}

{let s = x + (y * z
// c6
)}

{@html a || (b
// c7
)}

<p>{f(() => x + (y
// c8
))}</p>

<p>{a || (b
// c9
) // d9
}</p>

{#each f(() => x + (y
// c10
)) as item}
	<p>{item}</p>
{/each}

<input bind:value={() => v, (w) => x - (y
// c11
)} />

<input bind:value={() => x || (y // c12
), (w) => (x = w)} />

{#if cond}
	{@const f = // lead
	(a) => (b) => x || (y
	// c13
	)}
	{f}
{/if}

<input bind:value={() => x || (y
// c14
) // d14
, (w) => (x = w)} />

<!-- the run reaches the value's end only while the member chain stays flat -->
<p class={(a || (b
// c15
)).m}>t</p>
