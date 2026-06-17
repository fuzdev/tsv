{#await promise then {a, b = 1}}
	<div>{a} {b}</div>
{/await}

{#await promise}
	loading
{:then {a, b = "x"}}
	<div>{a} {b}</div>
{/await}

{#await promise}
	loading
{:then {a, b = 1}}
	<div>{a} {b}</div>
{:catch {msg, code = 0}}
	<div>{msg} {code}</div>
{/await}
