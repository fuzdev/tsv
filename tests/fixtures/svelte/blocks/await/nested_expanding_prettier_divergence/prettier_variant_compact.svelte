<a
	>{#await promise}{#if cond}text{/if}{/await}</a
>

<span
	>{#await promise}{#each items as item}text{/each}{/await}</span
>

<div>
	{#await promise}{#key id}text{/key}{/await}
</div>
