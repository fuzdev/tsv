<script lang="ts">
	let items;
	let cond;
</script>

<!-- inside the binding pattern: read once as an `as` type, once as a pattern -->
{#each items as { a } (a)}<div>{a}</div>{/each}

{#each items as { b }}<div>{b}</div>{/each}

<!-- inside the key parens, with a `, index` ahead of it -->
{#each items as c, i (/* c */ c)}<div>{c}{i}</div>{/each}

<!-- null control: the same key comment with no `, index` — listed once -->
{#each items as d (/* c */ d)}<div>{d}</div>{/each}

<!-- null control: an {#if} head takes no `as` unwind at all -->
{#if /* c */ cond}<div>text</div>{/if}
