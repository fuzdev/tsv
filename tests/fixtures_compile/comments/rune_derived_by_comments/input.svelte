<script>
	let n = $state(1);
	let double = $derived.by(/* compute */ () => n * 2);
	let total = $derived.by(() => {
		// sum it
		return n + double;
	});
	function inc() {
		n += 1;
	}
</script>

<button onclick={inc}>{double}{total}</button>
