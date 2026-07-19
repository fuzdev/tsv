<svelte:boundary>
	<p>x</p>
	{#snippet pending()}<i>w</i>{/snippet}
</svelte:boundary>

<style>
	p {
		color: red;
	}
	i {
		color: blue;
	}
</style>
