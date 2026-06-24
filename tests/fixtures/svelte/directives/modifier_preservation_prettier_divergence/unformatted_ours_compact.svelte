<div use:action|mod1|mod2></div><input bind:value|mod /><div class:class1|mod></div>
{#each items as item (item.id)}
<div animate:flip|mod></div>
{/each}
<slot let:item|mod></slot>
