{#snippet fn1(a = 'default')}
	<span>{a}</span>
{/snippet}

{#snippet fn2({ a, b })}
	<span>{a} {b}</span>
{/snippet}

{#snippet fn3([a, b])}
	<span>{a} {b}</span>
{/snippet}

{#snippet fn4({ a = 1 })}
	<span>{a}</span>
{/snippet}

{@render fn1()}
{@render fn2({ a: 1, b: 2 })}
{@render fn3([1, 2])}
{@render fn4({})}
