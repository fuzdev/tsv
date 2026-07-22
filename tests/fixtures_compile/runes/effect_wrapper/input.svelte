<script>
	let { prop } = $props();
	$effect(() => {
		console.log(prop);
	});
</script>

<p>{prop}</p>
