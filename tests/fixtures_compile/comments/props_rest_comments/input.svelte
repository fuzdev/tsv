<script>
	// the props
	let {
		label, // shown first
		// everything else
		...rest // forwarded
	} = $props();
	function describe() {
		// inner note
		return `${label}`;
	}
</script>

<p {...rest}>{describe()}</p>
