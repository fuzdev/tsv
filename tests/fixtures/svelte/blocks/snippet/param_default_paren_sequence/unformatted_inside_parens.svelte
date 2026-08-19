{#snippet fn1(a = (2, 3), b)}<span>{a} {b}</span>{/snippet}

{#snippet fn2(a, b = (4, 5))}<span>{a} {b}</span>{/snippet}

{#snippet fn3(a = (6, 7 /* c1 */), b)}<span>{a} {b}</span>{/snippet}

{#snippet fn4(a, b = (8, 9 /* c2 */))}<span>{a} {b}</span>{/snippet}

{@render fn1(1, 2)}
{@render fn2(1, 2)}
{@render fn3(1, 2)}
{@render fn4(1, 2)}
