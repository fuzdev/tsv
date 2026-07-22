<script>
	let x = $state(1);
</script>

<svelte:boundary>
	{@render inner()}{#snippet inner()}<p>{x}</p>{/snippet}{#snippet failed(e)}<b>{x}</b>{/snippet}
</svelte:boundary>
