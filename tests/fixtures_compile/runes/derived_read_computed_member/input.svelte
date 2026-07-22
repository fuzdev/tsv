<script>
	let { a, obj } = $props();
	let d = $derived(a * 2);
</script>

{obj[d]}
