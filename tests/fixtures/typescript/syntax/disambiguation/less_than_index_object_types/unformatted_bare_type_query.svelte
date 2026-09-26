<script lang="ts">
	// The indexed object may be any type that takes an index — a qualified name, `this`, a
	// type query, a keyword, a tuple, an object type, a parenthesized type — and the union
	// or intersection past the index continues the type argument all the same.
	const a1 = fn<A.B[K] | C>(x);
	const a2 = fn<this[K] | C>(x);
	const a3 = fn<this[0] & C>(x);
	const a4 = fn<typeof x[K] | C>(x);
	const a5 = fn<typeof x.y[0] & C>(x);
	const a6 = fn<string[K] | C>(x);
	const a7 = fn<[A, B][0] | C>(x);
	const a8 = fn<{ a: A }[K] | C>(x);
	const a9 = fn<(A | B)[K] | C>(x);

	// Objects whose index commits on its own, or that head the type another way.
	const b1 = fn<{ a: A }['a'] | C>(x);
	const b2 = fn<[A, B][number][] | C>(x);
	const b3 = fn<G<A>[K] | C>(x);
	const b4 = fn<keyof A[K] | C>(x);
	const b5 = fn<readonly A[K][] | C>(x);
</script>
