{#await promise catch error}{error}{:then value}{value}{/await}

{#await promise catch error}
	<p>{error}</p>
{:then value}
	<p>{value}</p>
{/await}

{#await promise catch}
	<p>text</p>
{:then value}
	<p>{value}</p>
{/await}
