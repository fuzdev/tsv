<!--
	The whitespace a block keyword requires is any JS `\s` character, so a Unicode
	space separates the keyword from its expression (see the unformatted_*
	variants). U+0085 NEL is not in JS `\s` and does not satisfy it.
-->
{#if cond}
	<p>text1</p>
{/if}

{#each items as item}
	<p>text2</p>
{/each}
