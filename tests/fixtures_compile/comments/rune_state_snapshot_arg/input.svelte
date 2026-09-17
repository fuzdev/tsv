<script>
	let s = $state({ x: 1 });
	let snap = $state.snapshot(/* frozen */ s);
	function set() {
		snap = null;
	}
</script>

<button onclick={set}>{snap}</button>
