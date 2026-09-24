<!-- array rest: a comment inside the parens the parser stripped from the binding -->
{#each items as [b, ...(a /* c */)]}<div>{a}</div>{/each}

<!-- object rest -->
{#each items as { ...(a /* c */) }}<div>{a}</div>{/each}

<!-- a line comment in the rest's parens: the // runs to end of line (no swallow) -->
{#each items as [b, ...(a // c
)]}<div>{a}</div>{/each}

{#each items as { ...(a // c
) }}<div>{a}</div>{/each}

<!-- a rename value and a default value carry the same stripped parens -->
{#each items as { b: (a /* c */) }}<div>{a}</div>{/each}

{#each items as { b = (1 /* c */) }}<div>{b}</div>{/each}

<!-- nested layers, and a shell before a following property -->
{#each items as { b: ((a /* c1 */) /* c2 */) }}<div>{a}</div>{/each}

{#each items as { b: (a /* c */), d }}<div>{a} {d}</div>{/each}

<!-- a line comment in a value's parens, before a following property: the comma drops
below it -->
{#each items as { b: (a // c
), d }}<div>{a} {d}</div>{/each}

<!-- an array element's default value -->
{#each items as [a = (1 /* c */), b]}<div>{a} {b}</div>{/each}

<!-- a line comment in the parens, then another after the `)`: each keeps its own line -->
{#each items as [b, ...(a // c1
) // c2
]}<div>{a}</div>{/each}

{#each items as { ...(a // c1
) // c2
 }}<div>{a}</div>{/each}

{#each items as { b: (a // c1
) // c2
, d }}<div>{a} {d}</div>{/each}
