<!-- a renamed (non-shorthand) property with a default keeps its source key `a:` -->
{#each items as { b = 1 }}<div>x</div>{/each}

<!-- the plain-rename sibling `c: d` is preserved alongside the defaulted one -->
{#each items as { b = 1, c: d }}<div>x</div>{/each}

<!-- a nested pattern with a default keeps its key too -->
{#each items as { { b } = c }}<div>x</div>{/each}
