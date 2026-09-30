<!-- a directive value may open on a regex: `{/` there is an expression, not a block close -->
<div class:class1={ /a/.test(b) }></div>
<div style:color={ /a/.source }></div>
<button on:click={ /a/ }>text</button>
<div use:action={ /a/ }></div>
<div transition:fade={ /a/ }></div>
{#each items as item (item.id)}
	<div animate:flip={ /a/ }></div>
{/each}
