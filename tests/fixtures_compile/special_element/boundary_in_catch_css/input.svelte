<script>
	let p = Promise.resolve(1);
</script>

{#await p}
	<p>l</p>
{:then v}
	<p>{v}</p>
{:catch e}
	<svelte:boundary>
		<p class="bc">{e}</p>
		{#snippet pending()}
			<i>w</i>
		{/snippet}
	</svelte:boundary>
{/await}

<style>
	.bc {
		color: red;
	}
</style>
