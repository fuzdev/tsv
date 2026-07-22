<script>
	let { a } = $props();
	let d = $derived(a * 2);
	function g() {
		const o = { d: 1 };
		return o.d + d;
	}
</script>

<button onclick={g}>{a}</button>
