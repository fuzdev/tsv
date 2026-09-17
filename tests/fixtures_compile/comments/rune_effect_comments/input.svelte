<script>
	let n = $state(1);
	$effect(() => {
		// log on change
		console.log(n);
	});
	$inspect(/* debug */ n);
	let label = 'count';
	function inc() {
		n += 1;
	}
</script>

<button onclick={inc}>{label} {n}</button>
