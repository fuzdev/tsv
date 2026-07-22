<script>
	let o = $props();
	let { a } = $derived.by(() => o);
</script>

{a}
