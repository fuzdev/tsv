<script lang="ts">
</script>

{#snippet fn(a: string, b: number)}
	<span>{a} {b}</span>
{/snippet}

{@render fn('text', 42)}
