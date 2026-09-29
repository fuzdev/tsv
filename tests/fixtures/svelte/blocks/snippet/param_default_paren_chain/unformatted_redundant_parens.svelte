{#snippet fn1(a = (((x || y)).z))}<span>{a}</span>{/snippet}

{#snippet fn2(a = ((x || y))())}<span>{a}</span>{/snippet}

{#snippet fn3(a = ((1)).toString())}<span>{a}</span>{/snippet}

{#snippet fn4(a = new ((f()))())}<span>{a}</span>{/snippet}

{#snippet fn5(a = ((b?.c)).d)}<span>{a}</span>{/snippet}

{#snippet fn6(a = ((() => 1))())}<span>{a}</span>{/snippet}

{#snippet fn7(a = ((x || y))`t`)}<span>{a}</span>{/snippet}
