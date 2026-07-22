<script>
	let a = $state(1);
	let b = $derived(a * 2);
	// note after derived
	let c = 3;
</script>

<p>{b}{c}</p>
