<p>
	{a, (b // c1
	)}
</p>

<div data-a={(a, (b // c2
))}></div>

{#if (a, (b // c3
))}
	<p>t</p>
{/if}

{@html (a, (b // c4
))}

{#if (a, (b // c5
)) ? d : e}
	<p>t</p>
{/if}
