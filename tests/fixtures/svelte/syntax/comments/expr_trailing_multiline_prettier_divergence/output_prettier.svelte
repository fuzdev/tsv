<!-- Trailing multi-line block comments in template expressions: tsv preserves them,
	rendered through the TS printer's multi-line block form (interior lines reindented
	to context for a `*`-aligned comment) -->
{a}

{#if cond}
	text1
{/if}

<input bind:value={val} />

{@debug x}

<!-- nested: interior lines sit at the expression's own indent -->
{#if a}
	{b}
{/if}

<!-- {@const}: the same reindent; prettier's own output here is additionally corrupt
	(an unmatched paren it then throws on) -->
{#each items as item}
	{@const y = item) /** c1
	 * c2
	 */}
	{y}
{/each}

<!-- a non-`*`-aligned interior is preserved verbatim, not reindented -->
{c}
