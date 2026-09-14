<script lang="ts">
	// An indentable block comment on its own line in a conditional type's `?`→branch run
	// re-indents its continuation lines to the branch's indent, like the comment's first line
	type A = B extends C
		? // c
			/* y
			 */
			D
		: E;

	// the `:` branch, a JSDoc-style block
	type F = G extends H
		? I
		: // c
			/** y
			 * z
			 */
			J;

	// a longer run: the block third
	type K = L extends M
		? // c1
			// c2
			/* y
			 */
			N
		: O;

	// an author blank above the block survives
	type P = Q extends R
		? // c

			/* y
			 */
			S
		: T;

	// the run's FIRST comment: a block the author broke after lands the same way
	type U = V extends W
		? /* y
			 */
			X
		: Y;

	// control: glued to the branch, the closing line carries it
	type Z = AA extends BB
		? /* y
			 */ CC
		: DD;
</script>
