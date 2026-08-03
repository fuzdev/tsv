<!-- a leading line comment breaks the head and indents its continuation; the trailing
	comment's own break is dedented, so the closing `}` still lands at the tag's own column -->
{#if // c1
cond}
	text
{/if}

<!-- the control: no leading comment, so nothing indents the content — the same `}` column,
	reached without a dedent -->
{#if cond}
	text
{/if}

<!-- the same closer rule one level in, on a second block kind -->
{#if cond}
	{#key // c1
	expr}
		text
	{/key}
{/if}

<!-- the break-after-operator layout indents a `{@const}` init the same way; its `}` lands at
	the tag's own column, not the init's -->
{#each items as item}
	{@const a = item && cond)} // c
{/each}
