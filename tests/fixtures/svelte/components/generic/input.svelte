<script lang="ts" generics="T">
	let items: T[] = $state([]);

	function add(item: T) {
		items.push(item);
	}
</script>

<ul>
	{#each items as item}
		<li>{item}</li>
	{/each}
</ul>
