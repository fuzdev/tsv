<script>
	let { obj } = $props();
</script>

{#await p}x{:catch e}{@render obj.method()}{/await}
