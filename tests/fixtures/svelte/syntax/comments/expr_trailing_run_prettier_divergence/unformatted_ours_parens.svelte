<!-- a trailing comment RUN keeps each comment where the author wrote it: the block comment
	follows the line comment's own break, so it starts that line instead of trailing a space -->
{@html expr // c
/* d */}

{foo // c
/* d */}

{#if cond // c
/* d */
}
	text
{/if}

<div
	{...expr // c
	/* d */}
></div>

<!-- a run that does not end in a line comment leaves no break for the `}` to reuse, so a
	directive value takes the block form instead of hugging -->
<button
	on:click={
		fn // c
		/* d */
	}
>
	text
</button>

<!-- a line comment inside parens the parser stripped, then another after the `)`: one run,
	so the second starts the line the first one broke -->
{(a // c1
) // c2
}

{#if (a // c1
) // c2
}
	text
{/if}

<div
	title={(a // c1
	) // c2
	}
></div>
