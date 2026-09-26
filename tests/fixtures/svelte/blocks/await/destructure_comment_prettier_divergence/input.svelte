<!-- then-shorthand: comment in a destructuring default preserved in place -->
{#await promise then { a = /* c */ 1 }}
	<div>{a}</div>
{/await}

<!-- :then and :catch branches preserve interior pattern comments too -->
{#await promise}
	loading
{:then { b /* c */ }}
	<div>{b}</div>
{:catch [.../* c */ rest]}
	<div>x</div>
{/await}

<!-- line comment in a then-shorthand pattern: it breaks the pattern, laid out like the same
pattern in a TypeScript declaration; the space before `then` is preserved -->
{#await promise then {
	d = // c
		1
}}
	<div>{d}</div>
{/await}
