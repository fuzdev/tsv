<script>
	let a = true;
	let b = true;
</script>

{#snippet menu()}content{/snippet}

{@render menu()}{#if a && b}, updated{/if}
