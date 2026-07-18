<script>
	let { n } = $props();
	const label = n // pick a branch
		? 'pos'
		: 'neg';
</script>

<p>{label}</p>
