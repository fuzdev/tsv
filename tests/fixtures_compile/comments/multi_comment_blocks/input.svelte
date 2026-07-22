<script>
	// first
	let a = 1;
	// second
	let b = 2;
</script>

{#if a}<p>{a}</p>{/if}
{#each [1] as n}<span>{b}</span>{/each}
