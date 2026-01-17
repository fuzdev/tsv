{#if a} <span>{value}</span>{/if}

{#if a} <span>{value}</span>{:else}
	<span>{other}</span>{/if}

{#if a} <span>{value}</span>{:else if b}
	<span>{other}</span>{/if}

{#if a} <span>{value}</span>
{:else}
	<span>{other}</span>{/if}

{#if a}<span>{value}</span>
{:else} <span>{other}</span>{/if}
