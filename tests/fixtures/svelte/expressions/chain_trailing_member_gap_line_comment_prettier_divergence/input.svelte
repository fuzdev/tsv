<script>
	// the same gap in a `<script>` keeps the sanctioned collapse: the deferred `//`
	// rides to the end of a line this document still owns
	fn().bar; // c9
</script>

<a
	href={fn() // c1
		.bar}
>
	t
</a>

<a
	{...fn() // c2
		.bar}
>
	t
</a>

<a
	style:color={fn() // c3
		.bar}
>
	t
</a>

{fn() // c4
	.bar}

{#if fn() // c5
	.bar
}
	p
{/if}

{@html fn() // c6
	.bar}

{#if cond}
	{@const v = fn() // c7
		.bar}
	{v}
{/if}

<input
	bind:value={
		fn() // c8
			.bar
	}
/>
