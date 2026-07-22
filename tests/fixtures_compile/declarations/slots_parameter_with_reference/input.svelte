<script>
	function f($$slots) {
		return $$slots;
	}
</script>

{#if $$slots.a}<p>{f(1)}</p>{/if}
