<script>
	let { x } = $props();
</script>

<Foo>
	{#if x}<p>a</p>{/if}
</Foo>
