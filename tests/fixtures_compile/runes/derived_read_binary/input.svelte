<script>
	let { a } = $props();
	let d = $derived(a * 2);
</script>

{d + 1}
