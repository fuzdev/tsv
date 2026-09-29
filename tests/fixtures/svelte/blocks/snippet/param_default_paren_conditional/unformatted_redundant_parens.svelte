{#snippet fn1(a = ((x ? y : z)) ? (w) : v)}<span>{a}</span>{/snippet}

{#snippet fn2(a = (x ? ((y = 1)) : z))}<span>{a}</span>{/snippet}

{#snippet fn3(a = x ? y : ((z ?? w)))}<span>{a}</span>{/snippet}

{#snippet fn4(a = (((x ? y : z)) + 1))}<span>{a}</span>{/snippet}

{#snippet fn5(a = ((x ? y : z)).w)}<span>{a}</span>{/snippet}
