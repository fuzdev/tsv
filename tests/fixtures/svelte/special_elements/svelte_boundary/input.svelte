<svelte:boundary onerror={fn}>
	<div>text</div>

	{#snippet pending()}
		<span>loading</span>
	{/snippet}

	{#snippet failed(error, reset)}
		<span>{error.message}</span>
		<button onclick={reset}>retry</button>
	{/snippet}
</svelte:boundary>
