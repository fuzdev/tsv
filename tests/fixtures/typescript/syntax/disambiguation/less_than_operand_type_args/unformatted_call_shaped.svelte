<script lang="ts">
	// Only a type NAME takes type arguments: a reference, a qualified name, a type query,
	// an import type. A `<` behind one opens the name's own argument list, inside the list
	// the first `<` opened.
	const a1 = fn<A | B<C>>(x);
	const a2 = fn<A[K] | B.C<D>>(x);
	const a3 = fn<typeof b<C>>(x);
	const a4 = fn<import('m').B<C>>(x);
	const a5 = fn<string.x<C>>(x);
	const a6 = fn<readonly A<C>[]>(x);

	// A keyword type, a literal type, an object or tuple type, a parenthesized type and an
	// indexed access take none, so a `<` behind one is the comparison operator and the
	// whole line a comparison chain — as a member and as the head. Each line is the chain
	// the call-shaped authoring in the `unformatted_call_shaped` variant reads as.
	const b1 = f<A | string<C>>(x);
	const b2 = f<A[K] | this<C>>(x);
	const b3 = f<A | null<C>>(x);
	const b4 = f<A | 'a'<C>>(x);
	const b5 = f<A | 1<C>>(x);
	const b6 = f<A | {}<C>>(x);
	const b7 = f<A | (B)<C>>(x);
	const b8 = f<'a'<C>>(x);
	const b9 = f<string<C>>(x);
	const b10 = f<[B]<C>>(x);
	const b11 = f<A[K]<C>>(x);
</script>
