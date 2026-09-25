<script lang="ts">
	let items;
</script>

{#each items as item: A ? B : C (item)}
	<p>{item}</p>
{/each}
