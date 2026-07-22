<svelte:element this={t} class="x"></svelte:element>
<p></p>
<span class="foo"></span>

<style>
	.x ~ .foo {
		color: green;
	}
</style>
