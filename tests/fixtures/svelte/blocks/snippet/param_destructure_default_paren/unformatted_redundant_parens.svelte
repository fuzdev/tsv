{#snippet fn1({ a = (((x + y)) * 2) })}<span>{a}</span>{/snippet}

{#snippet fn2([a = ((x ?? y)) || (z)])}<span>{a}</span>{/snippet}

{#snippet fn3([[a = (-((x ** 2)))]])}<span>{a}</span>{/snippet}

{#snippet fn4({ a: { b = (((x + y)) * 2) } })}<span>{b}</span>{/snippet}

{#snippet fn5({ [(((x || y)) && z)]: a })}<span>{a}</span>{/snippet}
