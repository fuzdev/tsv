<script lang="ts">
	// Arrow with call body that breaks - closing brackets separate
	fn(aaaa, (x) =>
		fn1(x, {
			a: b,
			c: d,
		}),
	);

	// Ternary body with breaking call - same pattern
	fn(aaaa, (x) =>
		cond
			? fn1(x, {
					a: b,
				})
			: null,
	);

	// Async arrow with await body - not expandable, expands all
	fn(
		aaaa,
		async (x) =>
			await fn1(x, {
				a: b,
				c: d,
			}),
	);

	// Short control - inline when call body fits
	fn(aaaa, (x) => fn1(x, {a: b}));

	// Short control - block body arrow hugs correctly
	fn(aaaa, () => {
		fn1(x, {
			a: b,
			c: d,
		});
	});
</script>
