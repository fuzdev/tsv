<script>
	let tag = 'div';
</script>

<!-- plain string attribute -->
<svelte:element this="hello"></svelte:element>

<!-- braced string expression: preserved with single quotes -->
<svelte:element this={"hello"}></svelte:element>

<!-- variable expression -->
<svelte:element this={tag}></svelte:element>
