<script>
	// a stable id
	const id = $props.id();
	function label() {
		// inner note
		return `label-${id}`;
	}
</script>

<p {id}>{label()}</p>
