<!-- an own-line block comment inside a call's arguments or an arrow's body breaks that value,
	and so the pattern -->
{#each items as { a = f(1
/* c */
) }}<div>{a}</div>{/each}

{#each items as { a = () => {
x;
/* c */
} }}<div>{a}</div>{/each}
