<!-- a computed key in an {#each} binding pattern keeps its [ ] brackets; braces hug (bracketSpacing: false) -->
{#each objs as {[id]: value}}<div>{value}</div>{/each}

<!-- a template-literal computed key keeps its brackets -->
{#each objs as {[`k${i}`]: value}}<div>{value}</div>{/each}

<!-- a computed key after a shorthand prop (not first) keeps its brackets -->
{#each objs as {a, [id]: value}}<div>{value}</div>{/each}

<!-- computed key with a nested object-pattern value and a rest sibling -->
{#each objs as {[id]: {a}, ...rest}}<div>{a}</div>{/each}

<!-- a computed key nested inside a computed key's value keeps brackets at every depth -->
{#each objs as {[a]: {[b]: c}}}<div>{c}</div>{/each}
