<!-- the list closes where Svelte's paren count does: a quote or `//` in a regex opens nothing -->
{#snippet a(s = /'/.test(t))}
	x
{/snippet}
{#snippet b(s = /"/.test(t), u = '"')}
	x
{/snippet}
{#snippet c(s = /`/.test(t))}
	x
{/snippet}
{#snippet d(n = /r// 2)}
	x
{/snippet}
