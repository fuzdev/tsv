<script lang="ts">
	let a: unknown[];
	let b: unknown;
	let cond: boolean;
</script>

<!-- a parenthesized last operand: the head expression ends at its `)` -->
{#each a || (b as A as item}
	<div>{item}</div>
{/each}

<!-- the same as a conditional's alternate -->
{#each cond ? a : (b as A as item}
	<div>{item}</div>
{/each}
