<!-- a newline-authored pending branch keeps its own line, empty, in the block form -->
{#await promise then value}
	{value}
{/await}

{#await promise catch error}
	{error}
{/await}

{#await promise then value}
	{value}
{:catch error}
	{error}
{/await}

<div>
	{#await promise then value}
		<p>a</p>
		<p>b</p>
	{/await}
</div>

<!-- a pending-only block keeps its empty line, and so does one whose every section drops -->
{#await promise}{/await}

<span>
	{#await promise}{/await}
</span>
