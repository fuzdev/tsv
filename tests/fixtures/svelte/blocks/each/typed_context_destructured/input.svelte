<script lang="ts">
	let items: Array<{ a: number; b: string }> = [];
	let pairs: Array<[number, string]> = [];
</script>

{#each items as { a, b }: { a: number; b: string }}
	<div>{a} {b}</div>
{/each}

{#each pairs as [n, s]: [number, string]}
	<div>{n} {s}</div>
{/each}
