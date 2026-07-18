<script>
	let { a } = $props();
	let d = $derived(a * 2);
	let snapshot = d;
</script>

<button onclick={() => snapshot}>{a}</button>
