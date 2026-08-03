{// c
foo}
{@html // c
expr}
{@render // c
fn()}
{@debug x}
{#if // c
cond}
	yes
{/if}
{#if a}
	a
{:else if // c
b}
	b
{/if}
{#each // c
items as item}
	{item}
{/each}
{#each items as item (// c
item.id)}{item}{/each}
{#await // c
promise then val}
	{val}
{/await}
{#key // c
expr}
	text
{/key}
{#each list as it}
	{@const y =
		// c
		it}
	{it}
{/each}
<div
	{...// c
	expr}
></div>
<input
	bind:value={
		// c
		val
	}
/>
<div
	{@attach // c
	fn}
></div>
<div
	data-attr={// c
	expr}
></div>
<button
	on:click={// c
	fn}
>
	text
</button>
<div
	class:class1={// c
	cond}
></div>
<div
	use:fn={// c
	expr}
></div>
<svelte:element
	this={// c
	tag}
></svelte:element>
<svelte:component
	this={// c
	Comp}
></svelte:component>
