<b>1</b><svelte:boundary>
	<p>a</p>
	{#snippet failed(e)}<i>A</i>{/snippet}
</svelte:boundary><b>2</b><svelte:boundary>
	<p>c</p>
	{#snippet failed(f)}<u>B</u>{/snippet}
</svelte:boundary>
