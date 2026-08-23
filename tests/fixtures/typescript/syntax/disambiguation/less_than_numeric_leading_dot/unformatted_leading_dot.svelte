<script lang="ts">
	// A numeric type-argument head needs no integer part: `.5` opens type
	// arguments exactly as `0.5` does. Prettier normalizes `.5` to `0.5`, so the
	// frozen rows below are where the `.`-led spelling survives formatting.
	// prettier-ignore
	const a1 = fn<.5>(t, u);
	// prettier-ignore
	const a2 = fn<.5 | .25>(t);

	// An indexed access asks the same question about its index, and a
	// `keyof`/`unique` operand about its operand.
	// prettier-ignore
	const a3 = fn<A[.5]>(t, u);
	// prettier-ignore
	const a4 = fn<keyof .5>(t, u);
	// prettier-ignore
	const a5 = fn<unique .5>(t, u);

	// The digit-led spellings those normalize to.
	const b1 = fn<.5>(t, u);
	const b2 = fn<.5 | .25>(t);
	const b3 = fn<-.5>(t);
	const b4 = fn<A[.5]>(t, u);
	const b5 = fn<keyof .5>(t, u);

	// A follow token that cannot continue a type keeps the `<` a comparison,
	// exactly as it does for a digit-led head, and an index that is arithmetic is
	// an array access rather than an indexed-access type.
	const c1 = x < .5 ? q : r > `t`;
	const c2 = x < .5 + 1 > (t, u);
	const c3 = x < .5.toString() > (t, u);
	const c4 = x < a[.5 + 1] > (t, u);
</script>
