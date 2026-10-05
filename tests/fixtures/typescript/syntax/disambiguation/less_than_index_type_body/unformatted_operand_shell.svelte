<script lang="ts">
	// An index whose body only the TYPE grammar can spell — an object or tuple type, a
	// function, construct or import type, a type operator — is an indexed-access type, so
	// the `<` opens type arguments exactly as it does behind a name index.
	const a1 = fn<A[{ a: 1 }]>(x);
	const a2 = fn<A[{}]>(x);
	const a3 = fn<A[[0]] | B>(x);
	const a4 = fn<A[[B, C]]>(x);

	// Function, construct and import types.
	const b1 = fn<A[() => B]>(x);
	const b2 = fn<A[(a: B) => C]>(x);
	const b3 = fn<A[<T>(a: T) => B]>(x);
	const b4 = fn<A[new () => B]>(x);
	const b5 = fn<A[abstract new () => B]>(x);
	const b6 = fn<A[import('m')]>(x);
	const b7 = fn<A[import('m').B]>(x);
	const b8 = fn<A[() => B | C]>(x);
	const b9 = fn<A[() => () => B]>(x);
	const b10 = fn<A[() => {}]>(x);
	const b11 = fn<A[(a) => a is B]>(x);
	const b12 = fn<A[() => [B]]>(x);
	const b13 = fn<A[(a) => asserts a]>(x);
	const b14 = fn<A[(a) => asserts a is B]>(x);
	const b15 = fn<A[(this: B) => asserts this]>(x);
	const b16 = fn<A[new <T>() => T]>(x);
	// `readonly` over a type query's array
	const b17 = fn<A[B | readonly (typeof b)[]]>(x);

	// Type operators.
	const c1 = fn<A[unique symbol]>(x);
	const c2 = fn<A[infer U]>(x);
	const c3 = fn<A[readonly B[]]>(x);
	const c4 = fn<A[keyof B | C]>(x);
	const c5 = fn<A[typeof b<C>]>(x);

	// The same bodies behind a second index, in a later member, and in a nested index.
	const d1 = fn<A[K][{ a: 1 }]>(x);
	const d2 = fn<B | A[() => C]>(x);
	const d3 = fn<A[A[import('m')]]>(x);
	const d4 = fn<A[B<C>[]]>(x);
	const d5 = fn<A[B<C> | D]>(x);
	const d6 = fn<A[B[C extends D ? E : F]]>(x);

	// The other followers that commit the list: a tagged template, a statement's end.
	const e1 = fn<A[{}]>`t`;
	const e2 = fn<A[() => B]>;

	// A paren shell around the body is read by what it holds; the shelled spellings ride
	// the `unformatted_operand_shell` variant.
	const f1 = fn<A[((B))]>(x);
	const f2 = fn<A[(B | C)]>(x);
	const f3 = fn<A[({})]>(x);
	const f4 = fn<A[(() => B)]>(x);
	const f5 = fn<A[(B)[]]>(x);
</script>
