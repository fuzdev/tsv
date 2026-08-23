<script lang="ts">
	// A unicode escape is an identifier character, so an escaped name opens type
	// arguments exactly as its decoded spelling does. Prettier decodes the escape,
	// so the frozen rows below are where the escaped spelling survives formatting.
	// prettier-ignore
	const a1 = fn<\u0054>(t, u);
	// prettier-ignore
	const a2 = fn<\u0054 | U>(t);
	// prettier-ignore
	const a3 = fn<T\u0055>(t, u);

	// An indexed access asks the same question about its index, an array type
	// about its element, and a `keyof` operand about its operand.
	// prettier-ignore
	const a4 = fn<A[\u0054]>(t, u);
	// prettier-ignore
	const a5 = fn<\u0054[]>(t, u);
	// prettier-ignore
	const a6 = fn<keyof \u0054>(t, u);

	// An escape continues a name, so a keyword an escape follows is an ordinary
	// identifier — `keyofA`, not the `keyof` operator.
	// prettier-ignore
	const a7 = fn<keyof\u0041>(t, u);

	// The braced spelling is the same identifier character.
	// prettier-ignore
	const a8 = fn<\u{54}>(t, u);
	// prettier-ignore
	const a9 = fn<A[\u{54}]>(t, u);

	// Every callee shape asks the same question of its head.
	// prettier-ignore
	const a10 = new fn<\u0054>(t, u);
	// prettier-ignore
	const a11 = a.b<\u0054>(t);
	// prettier-ignore
	const a12 = fn?.<\u0054>(t);

	// A qualified tail and a nested argument list continue the type, and `unique`
	// takes an operand exactly as `keyof` does.
	// prettier-ignore
	const a13 = fn<\u0054.B>(t, u);
	// prettier-ignore
	const a14 = fn<\u0054<X>>(t, u);
	// prettier-ignore
	const a15 = fn<unique \u0054>(t, u);

	// The word boundary holds for every operator keyword, not just `keyof`.
	// prettier-ignore
	const a16 = fn<typeof\u0041>(t, u);
	// prettier-ignore
	const a17 = fn<unique\u0041>(t, u);

	// The decoded spellings those normalize to.
	const b1 = fn<\u0054>(t, u);
	const b2 = fn<\u0054 | U>(t);
	const b3 = fn<T\u0055>(t, u);
	const b4 = fn<A[\u{54}]>(t, u);
	const b5 = fn<\u0054[]>(t, u);
	const b6 = fn<keyof \u0054>(t, u);
	const b7 = fn<keyof\u0041>(t, u);
	const b8 = new fn<\u0054>(t, u);
	const b9 = a.b<\u0054>(t);
	const b10 = fn?.<\u0054>(t);
	const b11 = fn<\u0054.B>(t, u);
	const b12 = fn<\u0054<X>>(t, u);
	const b13 = fn<unique \u0054>(t, u);
	const b14 = fn<typeof\u0041>(t, u);
	const b15 = fn<unique\u0041>(t, u);

	// An escaped name past the would-be closing `>` starts an expression, so the
	// `<` is the less-than operator and the line is a comparison chain…
	// prettier-ignore
	const c1 = a < b > \u0041;

	// …and a follow token that cannot continue a type keeps it one too.
	// prettier-ignore
	const c2 = p < T\u0041 ? q : r > s;
</script>
