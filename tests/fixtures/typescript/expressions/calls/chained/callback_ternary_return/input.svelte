<script lang="ts">
	// short ternary - stays inline
	const a = items.map((x) => (x.cond ? x.a : x.b)).filter(Boolean);

	// ternary that breaks - hugs callback, trailing comma
	const b = items
		.map((x) =>
			x.condition
				? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				: bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
		)
		.filter(Boolean);

	// nested ternary
	const c = items
		.map((x) =>
			x.a
				? aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
				: x.b
					? bbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbbb
					: ccccccccccccccccccccccccccccccccccccccccccccc
		)
		.filter(Boolean);

	// ternary with object branches - stays inline
	const d = items
		.map((x) => (x.condition ? { a: x.a, b: x.b } : { a: x.c, b: x.d }))
		.filter(Boolean);
</script>
