<script>
	let o = $props();
	let { a, b } = $derived(o);
	const s = a + b;
</script>

{a}{b}{s}
