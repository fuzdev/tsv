<script>
	let n = $state(1);
	let d = $derived(n * 2);
	function inc() {
		n++;
	}
</script>

<Foo a={d} />
