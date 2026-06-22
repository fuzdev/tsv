<script lang="ts">
	// Curried arrow with generic - callback in body stays inline
	const a =
		(b) =>
		<T,>(fn) =>
			b.c((tx) => fn(new C({ a: tx, b: d })));

	// Body line is exactly 100 chars - callback stays inline
	const b =
		(c) =>
		<T,>(fn) =>
			c.d((tx) => fn(new E({ a: tx, b: ddddddddddddddddddddddddddddddddddddddddddddddddddddddd })));

	// Body line is 101 chars - callback wraps
	const c =
		(d) =>
		<T,>(fn) =>
			d.e((tx) =>
				fn(new F({ a: tx, b: dddddddddddddddddddddddddddddddddddddddddddddddddddddddd }))
			);

	// With return type and async
	const e =
		(b: A) =>
		async <T,>(fn: (c: B) => Promise<T>): Promise<T> =>
			b.c(async (tx) => fn(new C({ a: tx, b: d })));
</script>
