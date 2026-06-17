<!-- then-shorthand: renamed property with a default keeps its source key `a:` -->
{#await promise then { b = 1 }}
	<div>x</div>
{/await}

<!-- :then and :catch branches preserve the key the same way -->
{#await promise}
	loading
{:then { b = 1 }}
	<div>x</div>
{:catch { b = 1 }}
	<div>x</div>
{/await}
