<p>
	{(a, b)}
</p>

<div data-a={(a, b)}></div>

{#if (a, b)}
	<p>t</p>
{/if}

{@html (a, b)}

{#if (a, b) ? d : e} // c5
	<p>t</p>
{/if}
