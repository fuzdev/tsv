<script>
	let n = $state(1);
	const d = $derived(n * 2);
	class C {
		x = $state(d + 1);
	}
	const c = new C();
</script>

<p>{c.x}</p>
