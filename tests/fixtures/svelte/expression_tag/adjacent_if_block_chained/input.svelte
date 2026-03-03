<script>
	let a = true;
	let b = true;
	let items = [1, 2, 3];
</script>

{fn(a || b)}{#if a && b}, text{/if}{#each items as item}, {item}{/each}
