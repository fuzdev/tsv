<svelte:boundary>
	<p>hi</p>
	{#snippet pending()}<span>load</span>{/snippet}{#snippet failed(e)}<b>{e}</b>{/snippet}
</svelte:boundary>
