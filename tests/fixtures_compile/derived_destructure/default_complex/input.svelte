<script>
	let o = $props();
	function f() {
		return 1;
	}
	let { a = f() } = $derived(o);
</script>

{a}
