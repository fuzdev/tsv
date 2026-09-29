<!-- the pair holds the comment: Svelte rejects one before this head's `}` or `,` -->
{#each a || (b // c1
)}
	<p>x</p>
{/each}

{#each a || (b // c2
), i}
	<p>{i}</p>
{/each}

<!-- a rebalanced chain keeps its pair too -->
{#each a || (b || c // c3
)}
	<p>x</p>
{/each}

<!-- a sole hugged arrow's body -->
{#each f(() => a || (b // c4
))}
	<p>x</p>
{/each}

<!-- a same-line block stays inline, inside the pair -->
{#each a || (b /* c5 */)}
	<p>x</p>
{/each}

<!-- a pair followed by more of the value strips as anywhere else: its comma ends the line -->
{#each f(a || (b // c6
), 1)}
	<p>x</p>
{/each}

<!-- whatever follows the pair in the value: a member, an index -->
{#each (a || (b // c7
)).m}
	<p>x</p>
{/each}

{#each x[a || (b // c8
)], i}
	<p>{i}</p>
{/each}

<!-- nested redundant pairs keep the outermost, holding both comments -->
{#each a || ((b // c9
) // c10
)}
	<p>x</p>
{/each}

<!-- only a pair holding the comment is kept: a redundant pair ending an earlier chain strips -->
{#each f(() => x || (b)) || (c // c11
)}
	<p>x</p>
{/each}
