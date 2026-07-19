{#snippet fn1(a = ',', b)}<span>{a} {b}</span>{/snippet }

{#snippet fn2(a = [ 1, 2 ], b)}<span>{a} {b}</span>{/snippet }

{#snippet fn3(a = { x: 1, y: 2 }, b)}<span>{a} {b}</span>{/snippet }

{@render fn1(1, 2)}
{@render fn2(1, 2)}
{@render fn3(1, 2)}
