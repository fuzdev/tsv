<!--
	`clean_nodes` HOISTS a `{@debug}` / `<title>` out of its fragment BEFORE it trims, so such a
	node is invisible to the whitespace rules and the text beside it becomes the fragment's
	first/last node. Its edge run is therefore render-free — `{#if cond}text {@debug cond}{/if}`
	compiles to `text`, exactly like the glued authoring — so the run is trimmed and every
	authoring of the document reaches this one form.
	INTERIOR is the opposite case: with content on both sides the two runs MERGE into one space
	rather than vanishing, so a space survives there and the glued form is a different document.
-->
{#if cond}text {@debug cond}{/if}

{#if cond}{@debug cond} text{/if}

<!-- <title> inside <svelte:head> is hoisted the same way: its edge run trims -->
<svelte:head><title>text2</title> text3</svelte:head>

<!-- Interior: `a {@debug} b` renders `a b`, so one space is kept -->
{#if cond}a {@debug cond} b{/if}

<!-- The ROOT fragment is a fragment too: at its edge, a hoisted `{@debug}` trims the run -->
text1 {@debug cond}
