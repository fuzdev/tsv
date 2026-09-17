<script>
	let n = $state(1);
	let double = $derived(/* twice */ n * 2);
	let label = $derived(
		(() => {
			// build the label
			const parts = [n, double];
			return parts.join(', '); // joined
		})()
	);
	let info = $derived({
		// the raw count
		count: n,
		double /* derived */
	});
	function inc() {
		n += 1;
	}
</script>

<button onclick={inc}>{label} {info.count}</button>
