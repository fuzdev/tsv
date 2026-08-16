<!-- object literal: its `}` already dedents to base, so the tag's `}` continues on it -->
{#if {
	aaaaaaaaaaaaaaaaaaaa: 1,
	bbbbbbbbbbbbbbbbbbbb: 2,
	cccccccccccccccccccc: 3,
	ddddddddddddd: 4
}}
	x
{/if}

<!-- array literal: the `as` clause joins the dedented `]` too -->
{#each [
	aaaaaaaaaaaaaaaaaaaaaaaa,
	bbbbbbbbbbbbbbbbbbbbbbbb,
	cccccccccccccccccccccccc,
	dddddddd
] as x}
	{x}
{/each}

<!-- block-bodied arrow: same dedented `}` -->
{#key () => {
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa;
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb;
	cccccccccccccc;
}}
	x
{/key}

<!-- a prefix operator does not change the last line: the call's `)` still leads it -->
{#if !fn0000(
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
	cccccccccccccccccccccccccc
)}
	x
{/if}

<!-- an `{#each}` key answers the same question one level in -->
{#each xs as x ({
	aaaaaaaaaaaaaaaaaa: 1,
	bbbbbbbbbbbbbbbbbb: 2,
	cccccccccccccccccc: 3,
	dddddddddddd: 4
})}
	{x}
{/each}

<!-- control: a binary head ends on an operand, so its `}` still drops to its own line -->
{#if aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
	cccccccccccccccc
}
	x
{/if}

<!-- `import()` and a JSDoc cast own their parens outright -->
{#await import(
	'./mmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmmm'
)}
	x
{/await}

<!-- a prefix `await` is transparent to the question, like `!` -->
{#key await fn0000(
	aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb,
	cccccccccccccccccccccc
)}
	x
{/key}
