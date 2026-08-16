<!-- key head at 100: stays flat, only the body expands -->
{#each items as item (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa && bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb && cccccccc)}
	{item}
{/each}

<!-- key head at 101: wraps, the key's `)` drops to base and `}` follows it -->
{#each items as item (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
	ccccccccc
)}
	{item}
{/each}

<!-- member chain: wraps, same `)` drop -->
{#each items as item (item
	.filter((x) => x.aaaaaaaaaaaaaaaaaaaa)
	.map((x) => x.bbbbbbbbbbbbbbbbbbbbbb)
	.filter((x) => x.cccccccccccccccccccc)
)}
	{item}
{/each}

<!-- single call whose args wrap: its `)` already dedents to base, so `))}` hugs -->
{#each items as item (fn0000(
	aaaaaaaaaaaaaaaaaaaaaaaaaa,
	bbbbbbbbbbbbbbbbbbbbbbbbbb,
	cccccccccccccccccccccccccc
))}
	{item}
{/each}

<!-- degenerate no-`as` form: the key is part of the head expression, so the ONE
	dangling closer is the tag's `}` and the key's `)` stays glued -->
{#each items, i (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa &&
	bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb &&
	ccccccccccccc)
}
	{i}
{/each}
