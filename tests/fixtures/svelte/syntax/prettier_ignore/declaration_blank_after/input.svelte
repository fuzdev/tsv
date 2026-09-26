<!--
	An authored blank line after a frozen declaration tag survives, as it does after the tag
	unfrozen.
-->

<!-- the directive on its own line, inside a block body -->
{#if cond}
	<!-- prettier-ignore -->
	{@const   a = 1}

	text1
{/if}

<!-- the directive glued to the text before it, at the root -->
text2<!-- prettier-ignore -->{let   b = 2}

text3
