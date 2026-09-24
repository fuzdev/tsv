<!-- an array element, a property value and an object rest -->
{#each xs as [a.b]}
	<p>{a.b}</p>
{/each}
{#each xs as { k: a.c, ...a.d }}
	<p>{a.c}</p>
{/each}
