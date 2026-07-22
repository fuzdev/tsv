<script>
	let { a } = $props();
	let d1 = $derived(a * 2);
	let d2 = $derived(a * 3);
</script>

{d1 + d2}
