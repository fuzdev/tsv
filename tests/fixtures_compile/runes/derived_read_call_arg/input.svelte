<script>
	let { a } = $props();
	let d = $derived(a * 2);
	function f(x) {
		return x;
	}
</script>

{f(d)}
