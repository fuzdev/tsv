<script>
	let { n } = $props();
</script>

{#if n === 1}
	<p>one</p>
{:else if n === 2}
	<p>two</p>
{:else}
	<p>other</p>
{/if}
