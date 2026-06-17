{#each items as {a,b=1}}<div>{a} {b}</div>{/each}

{#each items as {a,b='x'}}<div>{a} {b}</div>{/each}

{#each items as {a=1,b=[1,2],c={x:1}}}<div>{a}</div>{/each}
