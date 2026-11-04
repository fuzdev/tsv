<!-- the alternate's pair holds the comment: Svelte rejects one before this head's `}` or `,` -->
{#each a ? (b // c1
) : (c // c2
)}
	<p>x</p>
{/each}

{#each a ? b : (c /* c3 */)}<p>x</p>{/each}

{#each a ? (b // c13
) : (c /* c14 */)}
	<p>x</p>
{/each}

{#each a ? (b /* c4 */) : (c /* c5 */), i}<p>{i}</p>{/each}

{#each a ? (b // c9
) : (c // c10
), i}
	<p>{i}</p>
{/each}

<!-- a nested conditional's alternate, and a nested conditional as the alternate -->
{#each a ? (b // c6
) : x ? d : (e // c7
)}
	<p>x</p>
{/each}

{#each a ? b : (x ? d : e /* c8 */)}<p>x</p>{/each}

{#each a ? b : (x ? d : e // c11
)}
	<p>x</p>
{/each}

{#each a ? b : (
// prettier-ignore
x  ?  d : e // c12
)}
	<p>x</p>
{/each}
