<script lang="ts">
	// A leading run whose multi-line block the author broke after: the block trails the
	// operator, the break after its `*/` survives, and the rest of the run leads the
	// branch on the continuation line.

	// `?` branch: the multi-line block, then a single-line block glued to the value
	type A = B extends C
		? (
		/* c1
		d1 */
		/* c2 */ D)
		: E;

	// the `:` branch, same shape
	type F = G extends H
		? I
		: (
		/* c3
		d3 */
		/* c4 */ J);

	// each comment on its own line, the single-line one first
	type K = L extends M
		? (
		/* c5 */
		/* c6
		d6 */
		N)
		: O;

	// the interior of the run glued, the author broke after it
	type P = Q extends R
		? /* c7
		d7 */ /* c8 */
			S
		: (T);

	// a blank line after the run survives
	type U = V extends W
		? (
		/* c9
		d9 */

		X)
		: Y;

	// a line comment ahead of a block glued to the value
	type Z = AA extends BB
		? (// c10
		/* c11 */ CC)
		: DD;

	// control: the whole run glued to the value keeps the value on the closing line
	type EE = FF extends GG
		? (
		/* c12 */ /* c13
		d13 */ HH)
		: II;

	// control: a single-line run the author broke after
	type JJ = KKaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa extends LLaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa
		? (
		/* c14 */
		/* c15 */
		MMaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaaa)
		: NNaaaaaaaaaaaaaaaa;
</script>
