<script>
	let { show } = $props();
</script>

{#if show}
	<p>yes</p>
{:else}
	<p>no</p>
{/if}
