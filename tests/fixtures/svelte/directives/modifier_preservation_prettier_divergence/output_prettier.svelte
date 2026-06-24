<div use:action></div>
<input bind:value />
<div class:class1></div>
{#each items as item (item.id)}
	<div animate:flip></div>
{/each}
<slot let:item></slot>
