<svelte:boundary>
	{#if cond}text{/if}
</svelte:boundary>

<svelte:boundary>
	{#each items as item}text{/each}
</svelte:boundary>

<svelte:boundary>{#await promise}text{/await}</svelte:boundary>
