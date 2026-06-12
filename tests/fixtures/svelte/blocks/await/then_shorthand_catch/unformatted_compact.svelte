{#await promise then value}{value}{:catch error}{error}{/await}

{#await promise then value}
<p>{value}</p>
{:catch error}
<p>{error}</p>
{/await}

{#await promise then value}
<p>{value}</p>
{:catch}
<p>text</p>
{/await}
