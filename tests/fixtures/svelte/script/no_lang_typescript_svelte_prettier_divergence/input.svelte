<script>
	let x: number = 1;
	let xs: string[] = [];
	let p = Promise.resolve('a');
</script>

{#snippet fn<T>(a: T)}
	{a}
{/snippet}

{#each xs as item: string}
	{item satisfies string}
{/each}

{#await p as Promise<string> then v}
	{v}
{/await}

{x as number}
