<script lang="ts">
	// A type keyword heading a `<` commits to type arguments only when what
	// follows it inside the angles could continue a type. `?`, `-`, `+`, and
	// `!=` cannot, so each line below is a comparison — even though the token
	// past the would-be closing `>` (a template tag, a parenthesized sequence)
	// does not itself start an expression.
	const a1 = p < string ? q : r > `t`;
	const a2 = p < void -1 > `t`;
	const a3 = p < null + 1 > (t, u);
	const a4 = p < bigint != 0 > `t`;
	const a5 = p < undefined ? q : r > (t, u);
	const a6 = p < this ? q : r > `t`;

	// The identifier arm answers the same question the same way.
	const b1 = p < zz ? q : r > `t`;

	// Tokens that CAN continue a type keep committing: a qualified tail and a
	// union read as instantiations, exactly as they do for an identifier head.
	const c1 = p<string.length>(t, u);
	const c2 = p<string | q>(t, u);

	// The type-operator keywords are keyword-then-operand and open type
	// arguments when an operand follows.
	const d1 = fn<typeof x>();
	const d2 = fn<keyof U>();

	// Literal heads answer the same follow-token question: a numeric, string,
	// template, tuple, or parenthesized operand followed by arithmetic or a
	// ternary is a comparison…
	const e1 = x < 1 + 2 > (t, u);
	const e2 = x < 'a' + 'b' > `t`;
	const e3 = x < `a` ? q : r > (t, u);
	const e4 = x < [1] ? q : r > (t, u);
	const e5 = x < (a, b) ? q : r > `t`;
	const e6 = x < -a > (t, u);

	// …while a type-continuing follow still commits.
	const f1 = fn<1 | 2>(t);
	const f2 = fn<[T] | null>(t);
	const f3 = fn<-1>(t);

	// An operator keyword with no operand after it is an ordinary value name —
	// a bare name or an arithmetic operand stays a comparison…
	const g1 = p < keyof ? q : r > `t`;
	const g2 = p < typeof x ? q : r > `t`;
	const g3 = p < readonly + 1 > (t, u);

	// …but keyof and unique take ANY type operand, so a literal after them
	// still opens type arguments.
	const h1 = p<keyof -1>`t`;
	const h2 = p<unique -1>(t, u);

	// `this.` is member access, never a type head.
	const i1 = x < this.a > (t, u);
</script>
