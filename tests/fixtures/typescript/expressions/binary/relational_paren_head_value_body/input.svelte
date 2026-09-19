<script lang="ts">
	// A `(` type-argument head grades its BODY, and a body that is a value rather than a
	// type is a comparison chain whatever stands past the closing `>` — a call or a tagged
	// template included. A paren shell both formatters strip is not part of the head
	// either, so the body it held is graded where the shell stood; the
	// `unformatted_operand_shell` variant carries the parenthesized spelling of each line
	// below, and the two authorings owe one verdict.
	const a1 = a < b() > (t, u);
	const a2 = a < this.b > (t, u);
	const a3 = a < b++ > (t, u);
	const a4 = a < ~b > (t, u);
	const a5 = a < delete b.c > (t, u);
	const a6 = a < new B() > (t, u);
	const a7 = a < b - 1 > (t, u);
	const a8 = a < b * c > (t, u);
	const a9 = a < b?.c > (t, u);
	const a10 = a < void b > (t, u);
	const a11 = a < -b > (t, u);
	const a12 = a < +1 > (t, u);
	const a13 = a < b() > `t`;

	// a tagged template, a class expression, a regex literal, an `infer` no binding name
	// follows, and a `>>` or `>>>` that closes no nested list are value bodies too
	const a14 = a < b`c` > (t, u);
	const a15 = a < b.c`d` > (t, u);
	const a16 = a < typeof b`c` > (t, u);
	const a17 = a < class {} > (t, u);
	const a18 = a < class B {} > (t, u);
	const a19 = a < class extends B {} > (t, u);
	const a20 = a < /b<c>/ > (t, u);
	const a21 = a < infer[b] > (t, u);
	const a22 = a < infer.b > (t, u);
	const a23 = a < b >> c > (t, u);
	const a24 = a < b >>> c > (t, u);

	// the same head whose body IS a type stays a type-argument list at the same followers
	const b1 = a<(B | C) & D>(t, u);
	const b2 = a<(() => B) | null>(t, u);
	const b3 = a<(B | C)[]>(t, u);
	const b4 = a<typeof b>(t, u);
	const b5 = a<(B extends C ? D : E)[]>(t, u);
	const b6 = a<(B | C) & D>`t`;
</script>
