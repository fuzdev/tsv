<!-- head + body fit: fully inline -->
{#if (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}x{/if}

<!-- head alone at 100: stays flat, only the body expands -->
{#if (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
	x
{/if}

<!-- head alone at 101: wraps at the comma, `}` dangles -->
{#if (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)}
	x
{/if}

<!-- `{#each}`: the `as` clause follows the wrapped head on the `}` line -->
{#each (expr_aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa, expr_bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) as item}
	{item}
{/each}
