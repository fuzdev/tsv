<script>
	// the promise
	let p = Promise.resolve(1);
</script>

{#await p then value}
	<p>{value}</p>
{/await}
