<script>
	let a = $state(1);
	let d = $derived(a * 2);
</script>

{$state.snapshot(d)}
