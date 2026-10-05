<script lang="ts">
	// An index in a relational chain's region is read as the region is: where its body
	// reads as a type, the region would parse as a type-argument list, so the chain keeps
	// a paren pair around its `<` operand.
	const a1 = a < b[c | d] > e;
	const a2 = a < b[typeof c] > d;
	const a3 = a < b[c<D>] > e;
	const a4 = a < b[() => c] > d;
	const a5 = a < b[(c) => c.d] > e;
	const a6 = a < b[import('m')] > c;
	const a7 = a < b[c][() => d] > e;

	// A `{` or `[` body is a type to a parser that grades no body, whatever it holds, as
	// the same two delimiters are at the head of the region.
	const b1 = a < b[{ c: 1 }] > d;
	const b2 = a < b[[c]] > d;
	const b3 = a < b[{ ...s }] > c;
	const b4 = a < b[[c++]] > d;

	// A body whose first operand is followed by a token that continues no type — to tsv's
	// parse and to acorn-typescript's — keeps the chain bare: arithmetic, a call, a
	// sequence, an assignment, an arrow function returning an expression, a unary operand,
	// a shift, a conditional, a dynamic import. Four of the nine are regions tsc still
	// claims past a line break: a shelled sequence or assignment (`c3`, `c4`), a shift its
	// type parser re-scans (`c7`) and an import type that takes any specifier (`c9`).
	const c1 = a < b[c + 1] > d;
	const c2 = a < b[c()] > d;
	const c3 = a < b[(c, d)] > e;
	const c4 = a < b[(c = d)] > e;
	const c5 = a < b[() => c + 1] > d;
	const c6 = a < b[-c] > d;
	const c7 = a < b[c << d] > e;
	const c8 = a < b[c ? d : e] > f;
	const c9 = a < b[import(c)] > d;

	// The body is read no further than that first operand. The first token that could
	// continue a type — a `<`, a `|` or `&`, a `[`, a `.` — takes the rest of the index as
	// it stands, as a string key and a `keyof` or `typeof` do outright, so the chain keeps
	// its pair whatever follows.
	const d1 = a < b[c < d] > e;
	const d2 = a < b[c < d > e] > f;
	const d3 = a < b[c<D>(e)] > f;
	const d4 = a < b[(c < d) | (e > [])] > f;
	const d5 = a < b[c | d()] > e;
	const d6 = a < b[c[d + 1]] > e;
	const d7 = a < b[c.d()] > e;
	const d8 = a < b['c' + d] > e;
	const d9 = a < b[typeof c + 1] > d;
	const d10 = a < b[keyof(c, d)] > e;
	const d11 = a < b[(1).c] > d;
	const d12 = a < b[import.meta] > c;

	// A call-shaped authoring whose index is an expression is one of these chains, with
	// whichever pair its body takes; the `unformatted_ours_call_shaped` variant carries
	// that spelling of each.
	const e1 = f < A[a.b()] > x;
	const e2 = f < A['a' + b] > x;
	const e3 = f < A[typeof b + 1] > x;
	const e4 = f < A[b[c + 1]] > x;
	const e5 = f < A[keyof(b, c)] > x;
	const e6 = f < A[() => b + 1] > x;
	const e7 = f < A[b + 1] > x;
	const e8 = f < A[a.b()] > `t`;
</script>
