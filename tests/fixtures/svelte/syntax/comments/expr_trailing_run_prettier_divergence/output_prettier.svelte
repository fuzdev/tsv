<!-- a trailing comment RUN keeps each comment where the author wrote it: the block comment
	follows the line comment's own break, so it starts that line instead of trailing a space -->
{@html expr}

{foo}

{#if cond}
	text
{/if}

<div {...expr}></div>

<!-- a run that does not end in a line comment leaves no break for the `}` to reuse, so a
	directive value takes the block form instead of hugging -->
<button on:click={fn}> text </button>
