<script lang="ts">
	let { a }: { a: string } = $props();
</script>

{#snippet row(x: string)}
	<p>{x}</p>
{/snippet}

{@render (row as (v: string) => void)(a)}
