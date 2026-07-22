<script>
	let { a, b = $bindable(1), c, d = $bindable() } = $props();
</script>

{a}{b}{c}{d}
