<script>
	// the name
	let name = 'world';
</script>

{#snippet greeting()}
	<p>hello {name}</p>
{/snippet}
{@render greeting()}
