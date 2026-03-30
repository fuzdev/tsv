<input bind:value="{a}" />
<input bind:value="{value}" />
<div class:class1="{cond}"></div>
<div style:color="{value}"></div>
<button on:click="{fn}">text</button>
<div use:action="{a}"></div>
{#each items as item (item.id)}
	<div animate:flip="{a}"></div>
{/each}
<div transition:fade="{a}"></div>
