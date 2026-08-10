<script lang="ts">
	interface A {
		a: string;
	}
	let items: unknown;
</script>

<!-- before the iterable -->
{#each /* c */ items as A[] as item}<div>{item}</div>{/each}

<!-- after the iterable, before the assertion's `as` -->
{#each items /* c */ as A[] as item}<div>{item}</div>{/each}

<!-- after the assertion type, before the binding's `as` -->
{#each items as A[] /* c */ as item}<div>{item}</div>{/each}

<!-- a line comment before the iterable takes the head's own-line form -->
{#each // c
	items as A[]
as item}
	<div>{item}</div>
{/each}
