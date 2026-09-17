<svelte:boundary>
	<p>a</p>
	{#snippet failed(e)}<b>x</b>{/snippet}
</svelte:boundary>
<svelte:boundary>
	<p>b</p>
	{#snippet failed(e)}<i>y</i>{/snippet}
</svelte:boundary>
