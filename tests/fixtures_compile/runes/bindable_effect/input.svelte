<script>
	let { value = $bindable(1) } = $props();
	$effect(() => {});
</script>

{value}
