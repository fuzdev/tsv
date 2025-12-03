{#each items as item}
	{@const a = item.b}
	{a}
{/each}

{#if cond}
	{@const a = b}
	{a}
{:else}
	{@const c = d}
	{c}
{/if}

{#snippet fn(a)}
	{@const b = a.c}
	{b}
{/snippet}

{#await promise then value}
	{@const a = value.b}
	{a}
{/await}

{#key expr}
	{@const a = b}
	{a}
{/key}

<Comp>
	{@const a = b}
	{a}
</Comp>
