<script>
	let { a } = $props();
	let d = $derived({ x: a });
</script>

{d.x}
