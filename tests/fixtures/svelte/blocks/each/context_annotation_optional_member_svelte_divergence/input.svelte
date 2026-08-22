<script lang="ts">
	let xs: Array<{ a?: number }> = [];
</script>

{#each xs as e: { a?: number }}
	{e.a}
{/each}
