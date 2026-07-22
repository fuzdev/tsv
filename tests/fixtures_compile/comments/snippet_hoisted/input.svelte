<script>
	// unused by the snippet
	let x = 1;
</script>

{#snippet fixed()}
	<p>static</p>
{/snippet}
{@render fixed()}
<span>{x}</span>
