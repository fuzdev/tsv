{#each items as [a, b = 1]}<div>{a} {b}</div>{/each}

{#each items as [a, b = "x"]}<div>{a} {b}</div>{/each}
