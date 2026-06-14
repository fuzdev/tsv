{foo // c
}
{@html expr // c
}
{@render fn() // c
}
{@debug x // c
}
{#if cond // c
}yes{/if}
{#if a}a{:else if b // c
}b{/if}
{#each items // c
 as item}{item}{/each}
{#each items as item (item.id // c
)}{item}{/each}
{#await promise // c
 then val}{val}{/await}
{#key expr // c
}text{/key}
{#each list as it}{@const y = it // c
	}{it}{/each}
<div {...expr // c
}></div>
<input bind:value={val // c
} />
<div {@attach fn // c
}></div>
<div data-attr={expr // c
}></div>
<button on:click={fn // c
}>text</button>
<div class:class1={cond // c
}></div>
<div use:fn={expr // c
}></div>
