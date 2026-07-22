<script>
	let a = $state(1);
	let b = $derived(a * 2);
	// forces the wrapper
	let d = new Date();
</script>

<p>{b}{d}</p>
