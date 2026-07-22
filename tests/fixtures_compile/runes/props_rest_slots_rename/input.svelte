<script>
	let { ...rest } = $props();
</script>

{#if $$slots.a}<p>{rest}</p>{/if}
