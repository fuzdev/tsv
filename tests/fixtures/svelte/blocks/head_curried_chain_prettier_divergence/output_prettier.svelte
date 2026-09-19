<!-- A curried chain that fits stays on the head's line -->
{#if (a) => (b) => c}
	<p>text</p>
{/if}

<!-- A chain too long for the line wraps in the head: the heads past the first indent under it -->
{#if (aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa) => (bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb) => (cccccccccccccccccccccccccccccccccc) => ddddddddd}
	<p>text</p>
{/if}

<!-- A head that forces the break (a destructured parameter) breaks however short the chain is -->
{#if ({ a }) => (b) => c}
	<p>text</p>
{/if}

<!-- The same chain as a call argument in an each head -->
{#each items.map(({ a }) => (b) => c) as item}
	<p>{item}</p>
{/each}
