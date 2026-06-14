{#if cond/* c */}yes{/if}
{#if a}a{:else if b/* c */}b{/if}
{#each items/* c */ as item}{item}{/each}
{#each items as item (item.id/* c */)}{item}{/each}
{#await promise/* c */ then val}{val}{/await}
{#key expr/* c */}text{/key}
<div {...expr/* c */}></div>
<input bind:value={val/* c */} />
<div {@attach fn/* c */}></div>
<div data-attr={expr/* c */}></div>
{@html expr/* c */}
{@const x = expr/* c */}
<button on:click={fn/* c */}>text</button>
<div class:class1={cond/* c */}></div>
<div use:fn={expr/* c */}></div>
{foo/* c */}
{@render fn()/* c */}
<div style:color={expr/* c */}></div>
<div transition:fn={expr/* c */}></div>
<div animate:fn={expr/* c */}></div>
<Comp let:v={expr/* c */}></Comp>
