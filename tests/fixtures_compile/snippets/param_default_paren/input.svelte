{#snippet fn1(a = (x + y) * 2)}<span>{a}</span>{/snippet}

{#snippet fn2(a = -(x ** 2))}<span>{a}</span>{/snippet}

{#snippet fn3(a = () => ({}))}<span>{a}</span>{/snippet}

{@render fn1()}
{@render fn2()}
{@render fn3()}
