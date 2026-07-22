<script>
	let { value = $bindable() } = $props();
	const id = $props.id();
</script>

{value}{id}
