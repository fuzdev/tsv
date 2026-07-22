<script>
	let { name } = $props();
</script>

{#snippet greeting()}
	<p>Hello, {name}!</p>
{/snippet}

{@render greeting()}
