<div title={x}>text</div>

{#if x}yes{/if}

{@html x}

<button onclick={() => f(/** @type {D} */ (e) /* t4 */)}>go</button>

<div title={x}>text</div>

{#if x}
	yes
{/if}

{@html x}

<input bind:value={v} />

<div {...p}></div>

{#each xs as x}{x}{/each}

{#if x}
	{@const a = /** @type {K} */ (x)) /* t17 */ /* t18 */}
	{a}
{/if}

{@render r()}
