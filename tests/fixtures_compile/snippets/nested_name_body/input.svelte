<script>
	let v = 1;
</script>

<div>{#snippet a()}nested{/snippet}</div>
{#snippet a()}{v}{/snippet}{@render a()}
