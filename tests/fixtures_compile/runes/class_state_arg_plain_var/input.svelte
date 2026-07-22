<script>
	let n = $state(1);
	class C {
		x = $state(n);
	}
	const c = new C();
</script>

<p>{c.x}</p>
