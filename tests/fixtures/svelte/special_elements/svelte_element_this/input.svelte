<script>
	let tag = 'div';
	let x = 1;
</script>

<!-- plain string attribute -->
<svelte:element this="hello"></svelte:element>

<!-- variable expression -->
<svelte:element this={tag}></svelte:element>

<!-- string concatenation (not a Literal -> delegates to printJS) -->
<svelte:element this={'a' + 'b'}></svelte:element>

<!-- template literal -->
<svelte:element this={`hello`}></svelte:element>

<!-- template literal with interpolation -->
<svelte:element this={`a${x}b`}></svelte:element>
