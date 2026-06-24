<!-- comment trailing a param, before the comma -->
{#snippet fn1(a /* c */, b)}<span>x</span>{/snippet}

<!-- comment leading the first param -->
{#snippet fn2(/* c */ a, b)}<span>x</span>{/snippet}

<!-- comment trailing a middle param -->
{#snippet fn3(a, b /* c */, c)}<span>x</span>{/snippet}

<!-- a comment trailing each of several params -->
{#snippet fn4(a /* c */, b /* d */, c)}<span>x</span>{/snippet}

<!-- comment trailing the only param (no comma) -->
{#snippet fn5(a /* c */)}<span>x</span>{/snippet}

<!-- dangling comment in empty parens -->
{#snippet fn6(/* c */)}<span>x</span>{/snippet}
