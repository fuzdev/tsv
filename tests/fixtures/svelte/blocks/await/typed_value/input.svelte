<script lang="ts">
	let promise = Promise.resolve(42);
</script>

{#await promise}
	<p>text</p>
{:then value: number}
	<p>{value}</p>
{:catch error: Error}
	<p>{error}</p>
{/await}
