{@render inner()}<svelte:boundary>
	<p>a</p>
	{#snippet pending()}{#snippet inner()}<i>q</i>{/snippet}x{/snippet}
</svelte:boundary>
