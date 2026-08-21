<script lang="ts">
	interface A<T> {
		a: T;
	}
	let fns: unknown;
</script>

<!-- function type inside a type argument -->
{#each (fns as A<() => string>) as item}
	<div>{item}</div>
{/each}

<!-- parenthesized function type in an array type -->
{#each (fns as (() => string)[]) as item}
	<div>{item}</div>
{/each}

<!-- function type nested in a type literal -->
{#each (fns as A<{ a: () => string }>) as item}
	<div>{item}</div>
{/each}

<!-- assertion chain whose last type holds a function type, with an index -->
{#each (fns as unknown as A<() => string>) as item, i}
	<div>{i}: {item}</div>
{/each}
