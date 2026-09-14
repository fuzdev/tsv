<script lang="ts">
	// A run the author GLUED — a multi-line block ahead of a single-line one — and then
	// broke after, at an `as`/`satisfies` cast: the run and the type hang one level under
	// the keyword, the type dropping below the run's closing `*/`. Prettier relocates the
	// run's tail before the keyword (`x /* c2 */ as /* c1⏎d1 */ B`).

	// `as`
	const a = x /* c2 */ as /* c1
	d1 */ B;

	// `satisfies`
	const c = x /* c4 */ satisfies /* c3
	d3 */ D;

	// control: a lone multi-line block the author broke after hangs the type the same way
	const e = x as /* c5
	d5 */
	F;

	// a `//` TAIL after the multi-line block: prettier welds it into the block's first line
	const g = x as /* c6 // c7
	d6 */ H;

	// the same with a block between: prettier relocates the block and welds the `//`
	const i = x /* c9 */ as /* c8 // c10
	d8 */ J;
</script>
