<script>
	// the label
	let { label } = $props();
	function has_default() {
		// inner note
		return !!$$slots.default;
	}
</script>

<p>{label}{has_default()}</p>
