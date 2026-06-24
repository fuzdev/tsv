<!-- object default: comment after the `=` -->
{#each items as { a = 1 }}<div>{a}</div>{/each}

<!-- object default: comment before the `=` (stays on the binding) -->
{#each items as { b = 1 }}<div>{b}</div>{/each}

<!-- object: leading (after {), between properties, and trailing (before }) -->
{#each items as { c, d }}<div>{c} {d}</div>{/each}

<!-- object rename key:value: comment before and after the colon -->
{#each items as { e: f }}<div>{f}</div>{/each}

{#each items as { g: h }}<div>{h}</div>{/each}

<!-- object computed key: comment inside the brackets -->
{#each items as { [k]: v }}<div>{v}</div>{/each}

<!-- object rest: comment between ... and the binding -->
{#each items as { ...rest }}<div>x</div>{/each}

<!-- array: leading (after [), between elements, trailing (before ]) -->
{#each items as [m, n]}<div>{m} {n}</div>{/each}

<!-- array rest -->
{#each items as [...tail]}<div>x</div>{/each}

<!-- nested: comment in an inner object default -->
{#each items as [{ p = 1 }]}<div>{p}</div>{/each}

<!-- default VALUE is an object/array expression: interior comment preserved, kept inline -->
{#each items as { r = { s: 1 } }}<div>{r}</div>{/each}

{#each items as { t = [1] }}<div>{t}</div>{/each}

<!-- line comment: a // runs to end of line, so the pattern tail drops to the next line (no swallow) -->
{#each items as { q = 1 }}<div>{q}</div>{/each}
