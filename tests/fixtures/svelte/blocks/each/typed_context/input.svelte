<script lang="ts">
	let items: number[] = [1, 2, 3];
	let promise = Promise.resolve(42);
</script>

{#each items as item: number}
	<div>{item}</div>
{/each}
