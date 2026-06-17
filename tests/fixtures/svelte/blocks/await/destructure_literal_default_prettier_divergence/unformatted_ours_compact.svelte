<!-- then-shorthand: string default normalizes to single quote -->
{#await promise then {a="x"}}
	<div>{a}</div>
{/await}

<!-- :then branch: numeric default normalizes to canonical form -->
{#await promise}
	loading
{:then {a=0xFF}}
	<div>{a}</div>
{/await}

<!-- :catch branch: string default keeps double quotes (escape-minimizing) -->
{#await promise}
	loading
{:then {a=1}}
	<div>{a}</div>
{:catch {a="a'b"}}
	<div>{a}</div>
{/await}
