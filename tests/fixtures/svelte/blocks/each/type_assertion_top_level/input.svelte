<script lang="ts">
	interface A {
		a: string;
		b: number;
	}
	let items: unknown;
</script>

<!-- as with array type, index, and key -->
{#each items as A[] as item, i (item.a)}
	<div>{item.a}</div>
{/each}

<!-- as with simple type -->
{#each items as A as item}
	<div>{item}</div>
{/each}

<!-- as with array type and destructuring -->
{#each items as A[] as { a, b }}
	<div>{a}: {b}</div>
{/each}
