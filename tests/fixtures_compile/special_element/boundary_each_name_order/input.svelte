<svelte:boundary>
	{#each [1] as a}<p>{a}</p>{/each}{#snippet pending()}{#each [2] as b}<i>{b}</i
			>{/each}{/snippet}{#snippet failed(e)}{#each [3] as c}<u>{c}</u>{/each}{/snippet}
</svelte:boundary>{#each [4] as d}<s>{d}</s>{/each}
