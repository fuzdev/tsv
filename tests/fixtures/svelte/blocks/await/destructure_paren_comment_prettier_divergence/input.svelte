<!-- then-shorthand: a comment inside the parens the parser stripped from a rest binding -->
{#await promise then [b, ...a /* c */]}
	<div>{a}</div>
{/await}

<!-- :then and :catch branches: a rename value's and a rest's stripped parens -->
{#await promise}
	loading
{:then { b: a /* c */ }}
	<div>{a}</div>
{:catch { ...e /* c */ }}
	<div>{e}</div>
{/await}
