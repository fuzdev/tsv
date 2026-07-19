<svelte:boundary>
	<p>hi</p>
	{#snippet failed(error, reset)}<button onclick={reset}>{error.message}</button>{/snippet}
</svelte:boundary>
