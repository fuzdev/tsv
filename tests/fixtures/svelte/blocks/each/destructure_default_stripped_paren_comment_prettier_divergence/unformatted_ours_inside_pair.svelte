<!-- the default's stripped pair holds an own-line block: it flushes behind the default -->
{#each items as { v = a || (b
/* c1 */
) }}
	<p>{v}</p>
{/each}

{#each items as [v = a || (b
/* c2 */
)], i}
	<p>{v}</p>
{/each}

{#each items as { v = a || (b
/* c3 */
) } (v)}
	<p>{v}</p>
{/each}

{#each items as { w: { v = a || (b
/* c4 */
) } }}
	<p>{v}</p>
{/each}

<!-- a kept pair around the stripped one, and a hugged arrow's body -->
{#each items as { v = a + (q + (b
/* c5 */
)) }}
	<p>{v}</p>
{/each}

{#each items as { v = f(() => a || (b
/* c6 */
)) }}
	<p>{v}</p>
{/each}

<!-- the bare control: nothing in the pair, the pattern stays inline -->
{#each items as { v = a || b }}
	<p>{v}</p>
{/each}
