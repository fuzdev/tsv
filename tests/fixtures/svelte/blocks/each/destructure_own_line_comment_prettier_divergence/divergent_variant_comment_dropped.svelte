<!-- object: an own-line line comment after the last property, a rest, a default -->
{#each items as { a }}<div>{a}</div>{/each}

{#each items as { ...a }}<div>{a}</div>{/each}

{#each items as { a = 1 }}<div>{a}</div>{/each}

<!-- object: own-line before the first property and between two -->
{#each items as { a }}<div>{a}</div>{/each}

{#each items as { a, b }}<div>{a} {b}</div>{/each}

<!-- a line comment glued to the opening delimiter keeps that line -->
{#each items as { a }}<div>{a}</div>{/each}

{#each items as [a]}<div>{a}</div>{/each}

<!-- array: last element, rest, between two, after a hole -->
{#each items as [a]}<div>{a}</div>{/each}

{#each items as [...a]}<div>{a}</div>{/each}

{#each items as [a, b]}<div>{a} {b}</div>{/each}

{#each items as [, a]}<div>{a}</div>{/each}

<!-- an empty pattern holding only a line comment -->
{#each items as { }}<div>x</div>{/each}

{#each items as []}<div>x</div>{/each}

<!-- nested: the inner pattern breaks, so the outer one does too -->
{#each items as { a: { b } }}<div>{b}</div>{/each}

<!-- a run of two, an author blank above the comment, and one between two entries -->
{#each items as { a }}<div>{a}</div>{/each}

{#each items as { a }}<div>{a}</div>{/each}

{#each items as { a, b }}<div>{a} {b}</div>{/each}

<!-- own-line block comments break the pattern too -->
{#each items as { a }}<div>{a}</div>{/each}

{#each items as { a }}<div>{a}</div>{/each}

{#each items as { a, b }}<div>{a} {b}</div>{/each}

{#each items as [a]}<div>{a}</div>{/each}

<!-- an own-line block comment inside an object or array default breaks it, and so the pattern -->
{#each items as { a = { b: 1 } }}<div>{a}</div>{/each}

{#each items as { a = [1] }}<div>{a}</div>{/each}

<!-- an own-line block comment at a default object's key gap collapses onto the key's line,
	and the object it opened stays open -->
{#each items as { a = { b: 1 } }}<div>{a}</div>{/each}

<!-- a multi-line block comment breaks it as well -->
{#each items as { a }}<div>{a}</div>{/each}

<!-- the same-line authoring: only the comment's line differs -->
{#each items as { a }}<div>{a}</div>{/each}

{#each items as { a, b }}<div>{a} {b}</div>{/each}

<!-- an index, an index and a key, and a key alone follow the closing brace -->
{#each items as { a }, i}<div>{a}{i}</div>{/each}

{#each items as { a }, i (a)}<div>{a}{i}</div>{/each}

{#each items as { a } (a.id)}<div>{a}</div>{/each}

<!-- nested in an element: the pattern indents from the block's own column -->
<div>
	{#each items as { a }}<div>{a}</div>{/each}
</div>

<!-- nested in a block -->
{#if ok}{#each items as { a }}<div>{a}</div>{/each}{/if}
