<!-- then-shorthand: renamed property with a default keeps its source key `a:` -->
{#await promise then { a: b = 1 }}
	<div>x</div>
{/await}

<!-- :then and :catch branches preserve the key the same way -->
{#await promise}
	loading
{:then { a: b = 1 }}
	<div>x</div>
{:catch { a: b = 1 }}
	<div>x</div>
{/await}
