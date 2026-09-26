<!-- then-shorthand: an own-line comment after the last property breaks the pattern -->
{#await promise then {
	a
	// c
}}
	<div>{a}</div>
{/await}

<!-- :then and :catch branches: array and object, line and block -->
{#await promise}
	loading
{:then [
	a,
	// c
	b
]}
	<div>{a} {b}</div>
{:catch {
	...e
	/* c */
}}
	<div>{e}</div>
{/await}

<!-- a then-shorthand with its own binding, then a :catch pattern -->
{#await promise then x}
	{x}
{:catch {
	e
	// c
}}
	<div>{e}</div>
{/await}

<!-- the same-line authoring: only the comment's line differs -->
{#await promise then {
	a // c
}}
	<div>{a}</div>
{/await}

<!-- nested in an element -->
<div>
	{#await promise}
		loading
	{:then {
		// c
		a
	}}
		<div>{a}</div>
	{/await}
</div>
