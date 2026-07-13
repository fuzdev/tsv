<!-- Hug is all-or-nothing: a body welded to only the head, or only the tail, still lays
	out block-style. The boundary is render-free, so it cannot select the layout. -->
{#if a}<span>{value}</span>
{/if}

{#if a}<span>{value}</span>
{:else}<span>{other}</span>
{/if}

{#if a}<span>{value}</span>
{:else if b}<span>{other}</span>
{/if}
