<script lang="ts">
	// A `(` at a type-argument head opens a FUNCTION TYPE's parameter list when a
	// parameter START stands behind a `:`, `,`, `?` or `=` — or behind a `)` — and an
	// `=>` follows the list; the empty list and a leading `...` are parameter starts of
	// their own. `(…) =>` after a `<` is never an expression, since an arrow function
	// cannot be a relational operand, so each line below is a type-argument list and the
	// `<` is no comparison operator.
	const a1 = f<() => U>(x);
	const a2 = f<(...a: T[]) => U>(x);
	const a3 = f<(...a) => U>(x);
	const a4 = f<(a: T) => U>(x);
	const a5 = f<(a?: T) => U>(x);
	const a6 = f<(a?) => U>(x);
	const a7 = f<(a) => U>(x);
	const a8 = f<(a, b) => U>(x);
	const a9 = f<(a, b: T, ...c: U[]) => U>(x);
	const a10 = f<(this: T, a: U) => V>(x);
	const a11 = f<(a: T, b: U) => V>(x);

	// A destructuring pattern is a parameter start too, whatever it holds: the head scan
	// steps over the balanced group and reads the token past it.
	const b1 = f<({ a }: T) => U>(x);
	const b2 = f<({ a = b }: T) => U>(x);
	const b3 = f<({ a = b || c }: T) => U>(x);
	const b4 = f<({ a: b }: T) => U>(x);
	const b5 = f<([a]: T) => U>(x);
	const b6 = f<([a, ...b]: T) => U>(x);
	const b7 = f<([[a], b]: T) => U>(x);

	// The return type runs to the region's own `>`, so the scan carries a nested
	// argument list closing on `>>`, a conditional, another function type, and the
	// object / tuple / predicate forms.
	const c1 = f<(a: T) => Map<K, V>>(x);
	const c2 = f<(a: T) => Map<K, Set<V>>>(x);
	const c3 = f<(a: T) => A extends B ? C : D>(x);
	const c4 = f<(a) => (b) => c>(x);
	const c5 = f<(a: T) => { a: U }>(x);
	const c6 = f<(a: T) => [U, V]>(x);
	const c7 = f<(a: T) => a is U>(x);
	const c8 = f<(a?: T) => asserts a>(x);
	const c9 = f<(a: T) => readonly U[]>(x);
	const c10 = f<(a: T) => void>(x);
	const c11 = f<(a: 'x>y') => U>(x);

	// Every host the region sits in reads it the same way.
	const d1 = new C<(...a: T[]) => U>();
	const d2 = new Map<(...a: T[]) => U, V>();
	const d3 = f<(...a: T[]) => U>`t`;
	const d4 = a.b<(...a: T[]) => U>(x);
	const d5 = f<(...a: T[]) => U>;
	f<(...a: T[]) => U, V>(x);
	f<V, (...a: T[]) => U>(x);
	f<Array<(...a: T[]) => U>>(x);
	f?.<(...a: T[]) => U>(x);

	// The keyword heads take the same parameter lists.
	const e1 = f<new (...a: T[]) => U>(x);
	const e2 = f<abstract new (...a: any) => T>(x);
	const e3 = f<<T>(a: T) => T>(x);
	const e4 = f<<T>(...a: T[]) => T>(x);
</script>
