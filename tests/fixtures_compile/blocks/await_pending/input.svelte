<script>
	let { promise } = $props();
</script>

{#await promise}
	<p>loading</p>
{:then value}
	<p>{value}</p>
{:catch error}
	<p>failed</p>
{/await}
