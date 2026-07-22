<script>
	let { a } = $props();
	let d = $derived({ x: a });
</script>

<div title={d.x}></div>
