<script>
	let a = $state(/** @type {number | null} */ (null));
	let b = $state.raw(/* leading */ 1 /* trailing */);
	let c = $state(/* callee gap */ 2);
	let d = $state(
		3 // why three
	);
	function set() {
		a = 1;
		b = 2;
		c = 3;
		d = 4;
	}
</script>

<button onclick={set}>{a}{b}{c}{d}</button>
