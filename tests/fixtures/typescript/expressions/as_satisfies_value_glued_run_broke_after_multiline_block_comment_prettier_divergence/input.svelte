<script lang="ts">
	// A run the author GLUED — a multi-line block ahead of a single-line one — and then
	// broke after, at an `as`/`satisfies` cast: the run and the type hang one level under
	// the keyword, the type dropping below the run's closing `*/`. Prettier relocates the
	// run's tail before the keyword (`x /* c2 */ as /* c1⏎d1 */ B`).

	// `as`
	const a = x as /* c1
	d1 */ /* c2 */
		B;

	// `satisfies`
	const c = x satisfies /* c3
	d3 */ /* c4 */
		D;

	// control: a lone multi-line block the author broke after hangs the type the same way
	const e = x as /* c5
	d5 */
		F;

	// a `//` TAIL after the multi-line block: prettier welds it into the block's first line
	const g = x as /* c6
	d6 */ // c7
		H;

	// the same with a block between: prettier relocates the block and welds the `//`
	const i = x as /* c8
	d8 */ /* c9 */ // c10
		J;
</script>
