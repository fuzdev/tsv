<script lang="ts">
	let {
		rows,
		pairs,
		p
	}: {
		rows: Array<{ a: number; b: string }>;
		pairs: Array<[number, string]>;
		p: Promise<{ v: number }>;
	} = $props();
</script>

{#each rows as { a, b }: { a: number; b: string }}
	<li>{a} {b}</li>
{/each}

{#each pairs as [n, s]: [number, string]}
	<li>{n} {s}</li>
{/each}

{#await p then { v }: { v: number }}
	<p>{v}</p>
{/await}
