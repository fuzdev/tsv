{#await promise then value}{value}{:catch error}{error /* c */}{/await}

{#await promise then value}{value}{:catch error}{/* c */ error}{/await}

{#await promise then value}{value}{:catch error}<p>{error /* c */}</p>{/await}

{#await promise then value}
	{value}
{:catch error}
	{error // c
	}
{/await}
