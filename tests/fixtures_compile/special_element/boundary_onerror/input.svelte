<script>
	let n = 0;
</script>

<svelte:boundary
	onerror={(e, reset) => {
		n;
		reset();
	}}
>
	<p>hi</p>
</svelte:boundary>
