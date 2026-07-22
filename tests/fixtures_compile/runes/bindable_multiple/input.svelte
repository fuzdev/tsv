<script>
	let { value: v = $bindable(), open = $bindable(false) } = $props();
</script>

{v}{open}
