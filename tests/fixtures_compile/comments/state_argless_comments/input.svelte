<script>
	// unset
	let a = $state(/* nothing yet */);
	let b = $state(); // trailing
	function set() {
		// inner note
		a = 1;
		b = 2;
	}
</script>

<button onclick={set}>{a}{b}</button>
