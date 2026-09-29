{#snippet fn1(a = () => ({}))}<span>{a}</span>{/snippet}

{#snippet fn2(a = () => ({}).x)}<span>{a}</span>{/snippet}

{#snippet fn3(a = () => (x = 1))}<span>{a}</span>{/snippet}

{#snippet fn4(a = () => (x ? y : z))}<span>{a}</span>{/snippet}
