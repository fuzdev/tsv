<!-- a head's keyword is no operand, so a `/` right after it opens a regex -->
{#if /}/.test(a)}text{/if}
{#if !/}/.test(a)}text{:else if /}/.test(b)}text{/if}
{#key /}/.source}text{/key}
{#await /}/.source}text{/await}
{#each /}/.exec(a) as item}text{/each}
{@html /}/.source}
