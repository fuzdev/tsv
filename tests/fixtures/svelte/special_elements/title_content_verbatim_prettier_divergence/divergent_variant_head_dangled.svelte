<p>text1</p>

<svelte:head
	><title>text2 text3</title>

	{#if cond}
		<title>{expr} text4</title>
	{:else}
		<title></title>
	{/if}

	<div>text5</div></svelte:head
>
