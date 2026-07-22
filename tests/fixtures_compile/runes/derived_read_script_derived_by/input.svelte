<script>
	let { a } = $props();
	let d = $derived(a * 2);
	let e = $derived.by(d);
</script>

<button onclick={() => e}>{a}</button>
