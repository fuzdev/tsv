{foo}
{@html expr}
{@render fn()}
{@debug x}
{#if cond}
	yes
{/if}
{#if a}a{:else if b}b{/if}
{#each items as item}
	{item}
{/each}
{#each items as item (item.id)}{item}{/each}
{#await promise then val}
	{val}
{/await}
{#key expr}
	text
{/key}
{#each list as it}{@const y = it}{it}{/each}
<div {...expr}></div>
<input bind:value={val} />
<div {@attach fn}></div>
<div data-attr={expr}></div>
<button on:click={fn}>text</button>
<div class:class1={cond}></div>
<div use:fn={expr}></div>
