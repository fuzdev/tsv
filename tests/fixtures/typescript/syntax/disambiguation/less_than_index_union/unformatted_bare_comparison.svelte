<script lang="ts">
	// An indexed-access type argument followed by `|` or `&` opens type arguments: the
	// index is a type, and the operator continues it. Every index spelling answers
	// alike — a reference, a keyword, a numeric literal.
	const a1 = fn<A[K] | B>(x);
	const a2 = fn<A[K] & B>(x);
	const a3 = fn<A[number] | null>(null);
	const a4 = fn<A[0] | B>(x);
	const a5 = fn<A[-1] & B>(x);
	const a6 = fn<A[0.5] | B>(x);
	const a7 = fn<A[K] | B | C>(x);
	const a8 = fn<A[K] | B[K]>(x);
	const a9 = fn<A[0n] | B>(x);
	const a10 = fn<A[0x1] & B>(x);
	const a11 = fn<A[1e3] | B>(x);

	// A negative literal is a literal type however its sign is spaced, as a member or alone.
	const a12 = fn<A | -1>(x);
	const a13 = fn<-1>(x);

	// The same continuation behind an index that commits on its own: a string key, a
	// chained or array index, a later union member, a later argument.
	const b1 = fn<A['k'] | B>(x);
	const b2 = fn<A[K][J] | B>(x);
	const b3 = fn<A[K][] | B>(x);
	const b4 = fn<B | A[K]>(x);
	const b5 = fn<A[K]>(x);
	const b6 = fn<B, A[K] | C>(x);

	// A would-be closing `>` followed by an expression, or an index followed by a token
	// no type continues with, keeps the `<` a comparison.
	const c1 = a < b[c] | d > e;
	const c2 = a < b[c] || d > (e);
	const c3 = a < b[c] && d > (e);
	const c4 = a < b[c] ? d : e > (f);
	const c5 = a < b[c] - 1 > (d);
	const c6 = a < b[c].d > (e);
	const c7 = a < b[c] & d >= e;

	// A union or intersection member that is no type keeps the `<` a comparison, whatever
	// follows the would-be closing `>`.
	const d1 = a < b[c] | d() > (e);
	const d2 = a < b[c] | d ? e : f > (g);
	const d3 = a < b[c] | -d > (e);
	const d4 = a < b[c] | d + 1 > (e);
	const d5 = a < b[c] & d.e() > (f);
	const d6 = a < b | d() > (e);

	// A comparison that is itself an operand of a comparison.
	const r1 = (a < b[c] | d) > (e);
	const r2 = (a < b[c] & d) >= (e);

	// A comment between the index and the operator, or after the operator, is inside the
	// type argument list.
	const e1 = fn<A[K] /* c */ | B>(x);
	const e2 = fn<
		| A[K] // c
		| B
	>(x);
	const e3 = fn<A[K] | /* c */ B>(x);
</script>
