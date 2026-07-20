<script>
	let v = 1;
</script>

<div>{#snippet b()}nested{/snippet}</div>
{#snippet a()}{v}{/snippet}{@render a()}
