<script lang="ts">
	let items;
</script>

<!-- an identifier binding's type annotation -->
{#each items as item: A}<div>{item}</div>{/each}

<!-- a destructuring binding's type annotation -->
{#each items as { a }: B}<div>{a}</div>{/each}

<!-- the annotation of an indexed each, so the `, index` tail is in play -->
{#each items as b: C, i}<div>{b}{i}</div>{/each}
