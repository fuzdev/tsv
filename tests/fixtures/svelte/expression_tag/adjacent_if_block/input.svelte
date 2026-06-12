<script>
	let a, b;
</script>

{fn(a || b)}{#if a && b && a !== b}, updated {fn(b)}{/if}
