<script lang="ts">
	type T = string;
	let promise: unknown;
</script>

<!-- assertion in the head, block form -->
{#await (promise as Promise<T>)}
	<p>text</p>
{:then value}
	<p>{value}</p>
{/await}

<!-- assertion in the head, `then` shorthand -->
{#await (promise as Promise<() => T>) then value}
	<p>{value}</p>
{/await}

<!-- assertion chain, `catch` shorthand -->
{#await (promise as unknown as Promise<T>) catch error}
	<p>{error}</p>
{/await}
