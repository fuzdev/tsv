<!-- Trailing multi-line block comments in template expressions: tsv preserves them,
	rendered through the TS printer's multi-line block form (interior lines reindented
	to context for a `*`-aligned comment) -->
{a /** c1
 * c2
 */}

{#if cond /** c1
 * c2
 */
}
	text1
{/if}

<input
	bind:value={
		val /** c1
		 * c2
		 */
	}
/>

{@debug x /** c1
 * c2
 */}

<!-- nested: interior lines sit at the expression's own indent -->
{#if a}
	{b /** c1
	 * c2
	 */}
{/if}

<!-- {@const}: the same reindent; prettier's own output here is additionally corrupt
	(an unmatched paren it then throws on) -->
{#each items as item}
	{@const y = item /** c1
	 * c2
	 */}
	{y}
{/each}

<!-- a non-`*`-aligned interior is preserved verbatim, not reindented -->
{c /* n1
n2 */}
