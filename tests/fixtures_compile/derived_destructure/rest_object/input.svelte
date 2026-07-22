<script>
	let o = $props();
	let { a, ...r } = $derived(o);
</script>

{a}{r}
