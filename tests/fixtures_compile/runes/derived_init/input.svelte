<script>
	let a = $state(1);
	let d = $derived(a * 2);
	function inc() {
		a += 1;
	}
</script>

<p>{d}</p>
