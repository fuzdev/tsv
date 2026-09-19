<script lang="ts">
	// A `(` type-argument head grades its BODY, and a word that can only continue an
	// EXPRESSION refuses it — but only where that word stands in an operator position. The
	// same spellings are ordinary names everywhere else: a segment of a qualified name, a
	// type operator's operand, a string's content and a bare reference all keep reading as
	// type arguments. The `{` and `[` heads grade no body at all, so their lines here pin
	// the generic syntax the region has to carry rather than the grade; the `(` lines
	// below, and the `unformatted_operand_shell` variant's shelled spellings of the
	// strippable ones, are what the grade reads.
	const a1 = f<{ as: T; await: T; delete: T; in: T; instanceof: T; satisfies: T; yield: T }>(x);
	const a2 = f<{ as?: T; await?: T; in?: T; yield?: T }>(x);
	const a3 = f<{ delete(): void; in(): void; await<U>(): void }>(x);
	const a4 = f<{ a: T.in; b: T.as; c: ns.await.T }>(x);
	const a5 = f<{ a: typeof as; b: typeof await.b }>(x);
	const a6 = f<{ a: satisfies; b: in<T> }>(x);
	const a7 = f<{ readonly in: T; get delete(): T }>(x);
	const a8 = f<{ a: 'in'; b: T }>(x);
	const a9 = f<[T.in, T.as]>(x);
	const a10 = f<ns.await.T>(x);
	const a11 = f<{ [K in keyof T as `x${K}`]: T[K] }>(x);
	const a12 = f<{ a: in; as: T }>(x);

	// `this` heads no qualified type name, so `this.` refuses — but `typeof`'s operand IS
	// an entity name, and is the one type position that holds one.
	const b1 = f<{ a: typeof this.x }>(x);
	const b2 = f<{ a: keyof typeof this.y }>(x);
	const b3 = f<[keyof typeof this.x]>(x);
	const b4 = f<{ a: this }>(x);

	// The shapes a body-level grade has to carry past: a nested argument list closing on
	// `>>`, a mapped type's modifiers and `as` clause, a conditional type, tuple labels,
	// the signature members, and the literal types.
	const c1 = f<{ a: Map<K, Set<V>> }>(x);
	const c2 = f<[Map<K, Set<Array<V>>>]>(x);
	const c3 = f<{ -readonly [K in keyof T]-?: T[K] }>(x);
	const c4 = f<{ +readonly [K in keyof T]+?: T[K] }>(x);
	const c5 = f<A extends B ? 0.5 : C>(x);
	const c6 = f<{ a: A extends infer U extends string ? U : never }>(x);
	const c7 = f<[a: A, b?: B, ...c: C[]]>(x);
	const c8 = f<{ new (): T; (a: number): void; get x(): T; set x(v: T) }>(x);
	const c9 = f<abstract new () => T>(x);
	const c10 = f<[unique symbol, -1, 1n, 'a', `t${A}`]>(x);
	const c11 = f<A[]>(x);
	const c12 = f<[typeof import('x')]>(x);
	const c13 = f<() => T>(x);
	const c14 = f<(A | B) & C>(x);
	// An object type's members may be separated by a LINE BREAK alone, and a member name may
	// be one of the expression-only words. The `unformatted_newline_separated_members`
	// variant carries the `;`-less authoring of each.
	const d1 = f<{
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: T;
		as: U;
		instanceof: V;
		satisfies: W;
	}>(x);
	const d2 = f<{
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: T;
		as?: U;
		instanceof?(): V;
		satisfies?<Y>(): W;
	}>(x);
	const d3 = f<{
		aaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa: T;
		as(): U;
		instanceof<Y>(): V;
		readonly satisfies: W;
		get as(): X;
	}>(x);

	// The parenthesized type, which is the head the grade reads. A postfix `[`…`]` is the
	// only thing the type grammar carries past a `)`, so these are the shells prettier
	// keeps; the strippable ones ride the `unformatted_operand_shell` variant.
	const e1 = f<(typeof x)[number]>(x);
	const e2 = f<(keyof T)[]>(x);
	const e3 = f<(typeof this.x)[]>(x);
	const e4 = f<(keyof typeof this.y)[]>(x);
	const e5 = f<(A extends B ? C : D)[]>(x);
	const e6 = f<(abstract new () => T) | null>(x);
	const e7 = f<ns.await.T[]>(x);
	const e8 = f<(readonly T[])[]>(x);
	const e9 = f<(new () => T)[]>(x);
	const e10 = f<(A | B)[]>(x);
</script>
