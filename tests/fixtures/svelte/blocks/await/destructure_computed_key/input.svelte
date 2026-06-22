<!-- a computed key keeps its [ ] brackets in {#await … then} -->
{#await promise then { [k]: value }}
	<div>{value}</div>
{/await}

<!-- and in a {:then} clause -->
{#await promise}
	loading
{:then { [k]: value }}
	<div>{value}</div>
{/await}

<!-- and in a {:catch} clause -->
{#await promise}
	loading
{:then value}
	<div>{value}</div>
{:catch { [k]: error }}
	<div>{error}</div>
{/await}
