{#snippet fn1(a = (2, 3), b)}<span>{a} {b}</span>{/snippet}

{#snippet fn2(a, b = (4, 5))}<span>{a} {b}</span>{/snippet}

{@render fn1(1, 2)}
{@render fn2(1, 2)}
