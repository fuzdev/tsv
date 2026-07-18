<script>
	let { a } = $props();
	let d = $derived(a * 2);
	function total() {
		return d + 1;
	}
</script>

<button onclick={total}>{a}</button>
