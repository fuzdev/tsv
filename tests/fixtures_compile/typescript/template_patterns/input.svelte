<script lang="ts">
	let { items, p, n }: { items: number[]; p: Promise<string>; n: number } = $props();
</script>

{#each items as item: number}
	<li>{item}</li>
{/each}

{#await p then value: string}
	<p>{value}</p>
{:catch error: Error}
	<p>{error}</p>
{/await}

{#if n}
	{@const doubled: number = n * 2}
	<p>{doubled}</p>
{/if}
