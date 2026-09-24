<!-- then-shorthand: a comment inside the parens the parser stripped from a rest binding -->
{#await promise then [b, ...a]}
	<div>{a}</div>
{/await}

<!-- :then and :catch branches: a rename value's and a rest's stripped parens -->
{#await promise}
	loading
{:then { b: a }}
	<div>{a}</div>
{:catch { ...e }}
	<div>{e}</div>
{/await}
