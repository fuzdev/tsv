<svelte:element
	this={tag}
	onclick={() => {
		foo();
		bar();
	}}
/>

<svelte:element
	this={tag}
	onclick={() => {
		foo();
		bar();
	}}>content</svelte:element
>

<slot
	name="x"
	onclick={() => {
		foo();
		bar();
	}}>content</slot
>
