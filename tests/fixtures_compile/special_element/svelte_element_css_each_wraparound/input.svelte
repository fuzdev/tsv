{#each [1] as _}<svelte:element this={t} class="z"></svelte:element>{/each}

<style>
	.z + .z {
		color: green;
	}
	.z ~ .z {
		color: green;
	}
</style>
