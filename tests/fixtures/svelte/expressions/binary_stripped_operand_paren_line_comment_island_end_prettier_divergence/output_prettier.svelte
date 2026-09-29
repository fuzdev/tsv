<script>
	// the same pair in a `<script>`: the statement's own line end takes the comment
	const q = x + y * z; // c0
</script>

<p>
	{a || b || c}
</p>

<div data-a={x + y * z}></div>

{#if x + y * z}
	<p>t</p>
{/if}

{#each a instanceof b as item}
	<p>{item}</p>
{/each}

{#if cond}
	{@const r = x + y * z)} // c5
	{r}
{/if}

{let s = x + y * z;} // c6

{@html a || b}

<p>
	{f(() => x + y)}
</p>

<p>
	{a || b}
</p>

{#each f(() => x + y) as item}
	<p>{item}</p>
{/each}

<input bind:value={() => v, (w) => x - y} />

<input
	bind:value={
		() => x || y, // c12
		(w) => (x = w)
	}
/>

{#if cond}
	{@const f = // lead
		(a) => (b) => x || y)} // c13
	{f}
{/if}

<input
	bind:value={
		() => x || y, // c14
		// d14
		(w) => (x = w)
	}
/>

<!-- the run reaches the value's end only while the member chain stays flat -->
<p class={(a || b).m}>t</p>
