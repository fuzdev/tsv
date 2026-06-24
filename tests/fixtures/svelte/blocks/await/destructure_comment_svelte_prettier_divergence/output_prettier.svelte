<!-- then-shorthand: comment in a destructuring default preserved in place -->
{#await promise then { a = 1 }}
	<div>{a}</div>
{/await}

<!-- :then and :catch branches preserve interior pattern comments too -->
{#await promise}
	loading
{:then { b }}
	<div>{b}</div>
{:catch [...rest]}
	<div>x</div>
{/await}

<!-- line comment in a then-shorthand pattern: the // runs to end of line, so the tail
drops to the next line (no swallow); the space before `then` is preserved -->
{#await promise then { d = 1 }}
	<div>{d}</div>
{/await}
