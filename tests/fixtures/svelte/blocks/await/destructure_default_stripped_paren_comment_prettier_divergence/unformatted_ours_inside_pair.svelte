<!-- the default's stripped pair holds an own-line block: it flushes behind the default -->
{#await p then { v = a || (b
/* c1 */
) }}
	<p>{v}</p>
{/await}

{#await p}
	<p>x</p>
{:then [v = a || (b
/* c2 */
)]}
	<p>{v}</p>
{/await}

{#await p catch { v = a || (b
/* c3 */
) }}
	<p>{v}</p>
{/await}

<!-- the bare control: nothing in the pair, the pattern stays inline -->
{#await p then { v = a || b }}
	<p>{v}</p>
{/await}
