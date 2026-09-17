<script>
	// bindables
	let {
		// the value
		value = $bindable(/* none */), // trailing
		count = $bindable(0)
	} = $props();
	function reset() {
		// inner note
		value = undefined;
		count = 0;
	}
</script>

<button onclick={reset}>{value}{count}</button>
