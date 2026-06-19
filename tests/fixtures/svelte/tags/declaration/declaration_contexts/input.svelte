{#if cond}
	{const a = b}
	{let c = d}
	{a}
{/if}

{#each items as item}
	{const a = item.b}
	{a}
{/each}

{#snippet fn(x)}
	{let a = x.b}
	{a}
{/snippet}

<div>
	{const a = b}
	{a}
</div>

<Comp>
	{let a = b}
	{a}
</Comp>
