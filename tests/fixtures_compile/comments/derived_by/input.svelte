<script>
	let a = $state(1);
	let b = $derived.by(() => a * 2);
	// note after by
	let c = 3;
</script>

<p>{b}{c}</p>
