<script>
	let a = $state(1);
	// first
	let b = $derived(a * 2);
	// second
	let c = $derived(a + 1);
	// third
	let d = 3;
</script>

<p>{b}{c}{d}</p>
