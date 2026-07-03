<script lang="ts">
	let items: Array<(a: string) => void> = [];
</script>

{#each items as fn: (a: string) => void}
	<div>{fn}</div>
{/each}
