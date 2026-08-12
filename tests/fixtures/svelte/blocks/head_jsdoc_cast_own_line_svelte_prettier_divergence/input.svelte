{#if /** @type {A} */ (aa)}
	text1
{:else if /** @type {B} */ (bb)}
	text2
{/if}

{#each /** @type {C} */ (items1) as item1}
	text3
{/each}

{#each items2 as item2 (/** @type {D} */ (item2))}
	text4
{/each}

{#await /** @type {E} */ (promise1)}
	text5
{/await}

{#await /** @type {F} */ (promise2) then value}
	text6
{/await}

{#key /** @type {G} */ (cc)}
	text7
{/key}
