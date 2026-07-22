<script>
	let { a } = $props();
	let d = $derived(a * 2);
	let d2 = $derived(d + 1);
</script>

<button onclick={() => d2}>{a}</button>
