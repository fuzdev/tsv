<script>
	let x: number = 1;
	let xs: string[] = [];
</script>

{#snippet fn<T>(a: T)}
	{a}
{/snippet}

{#each xs as item: string}
	{item satisfies string}
{/each}

{x as number}
