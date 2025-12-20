{#each /* c */ items as item}{item}{/each}
{#each items as item (/* c */ item.id)}{item}{/each}
{#each items as item}{item}{:else}none{/each}
