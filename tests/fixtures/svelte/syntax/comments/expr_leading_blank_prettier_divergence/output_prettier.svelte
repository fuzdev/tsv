{// c1

foo}
{// c2

foo}
{@html // c3

expr}
{@html // c4

expr}
{@render // c5

fn()}
{@render // c6

fn()}
{#if // c7

cond}
	yes
{/if}
{#if // c8

cond}
	yes
{/if}
{#each // c9

items as item}
	{item}
{/each}
{#each // c10

items as item}
	{item}
{/each}
{#each list as it}
	{@const y =
		// c11

		it}
	{it}
{/each}
<div
	{...// c12

	expr}
></div>
<div
	{...// c13

	expr}
></div>
<input
	bind:value={
		// c14

		val
	}
/>
<div
	data-attr={// c15

	expr}
></div>
<div
	data-attr={// c16

	expr}
></div>
{@html // c17

// c18
expr}
{@html // c19

// c20
expr}
