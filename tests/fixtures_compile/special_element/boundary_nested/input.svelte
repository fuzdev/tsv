<svelte:boundary>
	<svelte:boundary>
		<p>a</p>
		{#snippet failed(f)}<i>in</i>{/snippet}
	</svelte:boundary>{#snippet failed(e)}<b>out</b>{/snippet}
</svelte:boundary>
