<svelte:boundary>
	{#each [1] as a}<p>{a}</p>{/each}{#snippet pending()}<i>w</i>{/snippet}
</svelte:boundary>{#each [2] as z}<u>{z}</u>{/each}
