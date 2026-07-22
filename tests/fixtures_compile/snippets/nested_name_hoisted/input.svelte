<script>
	let v = 1;
</script>

{#snippet a()}static{/snippet}
<div>{#snippet a()}{v}{/snippet}</div>
{@render a()}
