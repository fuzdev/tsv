<script>
	// Two callback calls with expression bodies stay inline
	x = items.filter((d) => d.a === b).map((d) => d.c);
	x = items.filter((d) => fn(d, b)).map((d) => d.c);
	x = items.map((d) => (d.a ? b : c)).filter((d) => d.c);
	x = items.filter((d) => d.a).map((d) => d.c);
	x = items.filter((d) => d.a).map((d) => d.c === b);
	x = items.filter((d) => d.a /* c */ === b).map((d) => d.c);

	// Declaration, statement and member-tail positions
	const y = items.filter((d) => d.a === b).map((d) => d.c);
	items.filter((d) => d.a === b).map((d) => d.c);
	items.filter((d) => d.a === b).map((d) => d.c).length;

	// An object body with a newline after its first key breaks the chain
	x = items
		.filter((d) => ({
			a: d
		}))
		.map((d) => d.c);

	// A multiline template body breaks the chain
	x = items
		.filter(
			(d) => `a
b`
		)
		.map((d) => d.c);

	// A block body breaks the chain
	x = items
		.filter((d) => {
			return d.a;
		})
		.map((d) => d.c);

	// A line comment in the body breaks the chain
	x = items
		.filter(
			(d) =>
				d.a === // c
				b
		)
		.map((d) => d.c);

	// A trailing line comment at the body end breaks the chain
	x = items
		.filter(
			(d) => d.a === b // c
		)
		.map((d) => d.c);

	// A block comment containing a newline breaks the chain
	x = items
		.filter(
			(d) =>
				d.a /* c
				 */ === b
		)
		.map((d) => d.c);

	// Width alone breaks the chain
	x = items
		.filter((d) => d.aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa === bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb)
		.map((d) => d.c);
</script>

<button onclick={() => items.filter((d) => d.a === b).map((d) => d.c)}>x</button>

{#each items.filter((d) => d.a === b).map((d) => d.c) as item}{item}{/each}
