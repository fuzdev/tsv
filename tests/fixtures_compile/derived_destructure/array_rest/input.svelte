<script>
	let o = $props();
	let [a, ...rest] = $derived(o);
</script>

{a}{rest}
