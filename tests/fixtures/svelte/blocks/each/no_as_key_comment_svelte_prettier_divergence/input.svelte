<!-- no `as` binding: leading comment inside the key parens -->
{#each items, i (/* c */ i)}<div>{i}</div>{/each}

<!-- no `as` binding: trailing comment inside the key parens -->
{#each items, i (i /* c */)}<div>{i}</div>{/each}

<!-- the `as` counterparts take the same two forms -->
{#each items as item, i (/* c */ item)}<div>{item}</div>{/each}

{#each items as item, i (item /* c */)}<div>{item}</div>{/each}
