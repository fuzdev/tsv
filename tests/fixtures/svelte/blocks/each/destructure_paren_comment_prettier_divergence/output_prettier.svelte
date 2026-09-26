<!-- array rest: a comment inside the parens the parser stripped from the binding -->
{#each items as [b, ...a]}<div>{a}</div>{/each}

<!-- object rest -->
{#each items as { ...a }}<div>{a}</div>{/each}

<!-- a line comment in the rest's parens breaks the pattern, as it breaks the TS twin -->
{#each items as [b, ...a]}
	<div>{a}</div>
{/each}

{#each items as { ...a }}
	<div>{a}</div>
{/each}

<!-- a rename value and a default value carry the same stripped parens -->
{#each items as { b: a }}<div>{a}</div>{/each}

{#each items as { b = 1 }}<div>{b}</div>{/each}

<!-- nested layers, and a shell before a following property -->
{#each items as { b: a }}<div>{a}</div>{/each}

{#each items as { b: a, d }}<div>{a} {d}</div>{/each}

<!-- a line comment in a value's parens, before a following property: the comma moves
ahead of it, as in every TypeScript list -->
{#each items as { b: a, d }}
	<div>{a} {d}</div>
{/each}

<!-- an array element's default value -->
{#each items as [a = 1, b]}<div>{a} {b}</div>{/each}

<!-- a line comment in the parens, then another after the `)`: each keeps its own line -->
{#each items as [b, ...a]}
	<div>{a}</div>
{/each}

{#each items as { ...a }}
	<div>{a}</div>
{/each}

{#each items as { b: a, d }}
	<div>{a} {d}</div>
{/each}

<!-- an own-line comment in the rest's parens, then a comment after the `)`: the own-line
comment keeps its line, and the run stays in the order the author wrote it -->
{#each items as [b, ...a]}
	<div>{a}</div>
{/each}

{#each items as { b, ...a }}
	<div>{a}</div>
{/each}
