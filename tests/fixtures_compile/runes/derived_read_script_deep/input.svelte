<script>
	let { a } = $props();
	let d = $derived(a * 2);
	function f() {
		if (a) {
			for (let i = 0; i < 3; i++) {
				console.log(d + i);
			}
		}
	}
</script>

<button onclick={f}>{a}</button>
