<script>
	let count = $state(0);

	$effect(() => {
		$inspect.trace();
		console.log(count);
	});

	$effect(() => {
		$inspect.trace('labeled');
		console.log(count * 2);
	});
</script>

<button onclick={() => count++}>Increment</button>
