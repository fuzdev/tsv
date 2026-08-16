<!-- head + body fit: fully inline -->
{#if (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}x{/if}

<!-- head alone at 100: stays flat, only the body expands -->
{#if (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
	x
{/if}

<!-- head alone at 101: operands indent under the head, `}` dangles -->
{#if (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
}
	x
{/if}

<!-- `{#each}`: the `as` clause follows the wrapped head on the `}` line -->
{#each (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
as item}
	{item}
{/each}

<!-- an `{#each}` key is a head of its own — at 100 it stays flat -->
{#each xs as x ((expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb))}
	{x}
{/each}

<!-- the same key at 101: same indent, and its `)` drops like a block head's `}` -->
{#each xs as x ((expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa,
	expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
)}
	{x}
{/each}
