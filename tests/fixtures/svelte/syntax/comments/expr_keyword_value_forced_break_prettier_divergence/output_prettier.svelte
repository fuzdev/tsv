<script lang="ts">
	let a, b, C;
</script>

<!-- a block run the author broke after, ending in a multi-line comment glued to the value, hangs
the rest of the run inside the braces -->
{await /* x */
/* y
 */ a}

{@html await /* x */
/* y
 */ b}

<div
	data-attr={new /* x */
	/* y
	 */ C()}
></div>

{#each [1] as item}
	{@const v = await /* x */
	/* y
	 */ b}
	{v}
{/each}

<!-- in a block head the run hangs under its keyword and the head's closer dangles -->
{#if a && new /* x */ /* y
	 */ C()}
	text
{/if}
