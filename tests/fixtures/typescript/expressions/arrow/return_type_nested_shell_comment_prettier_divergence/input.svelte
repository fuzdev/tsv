<script lang="ts">
	// a comment written last in a pair down the right edge of an arrow's parenthesized
	// return type stays inside the outer pair, as it does when written there
	x = (
		a
	): (
		B | C /* x1
		 */
	) => 1;

	x = (
		a
	): (
		B | C // x2
	) => 1;

	x = (
		a
	): (
		B & C /* x3
		 */
	) => 1;

	x = (
		a
	): (
		keyof C /* x4
		 */
	) => 1;

	x = (
		a
	): (
		A extends B ? C : D /* x5
		 */
	) => 1;

	x = (
		a
	): (
		B | C /* x6
		 */
	) => 1;

	// an inner pair that could stay is no authoring signal either
	x = (
		a
	): (
		B | C[] // x7
	) => 1;

	// a `//` after the `:` hangs the kept pair below it
	x = (
		a
	): // p8
		(
			B | C /* x8
			 */
		) => 1;

	// a pair the printer adds keeps the comment after it
	x = (
		a
	): (
		B | (C & D) // x9
	) => 1;

	// a generic reference keeps the outer pair, which a `(` cannot misread
	x = (
		a
	): (
		G<T> // x10
	) => 1;

	// a comment leading the inner pair stays ahead of the member
	x = (
		a
	): (
		| B
		// y11
		| C /* x11
		 */
	) => 1;

	// an operator member and a constructor type's return type keep the outer pair too
	x = (
		a
	): (
		B | keyof C /* x12
		 */
	) => 1;

	x = (
		a
	): (
		B | readonly C[] // x13
	) => 1;

	x = (
		a
	): (
		new () => C // x14
	) => 1;

	// a `//` glued to the inner `(` keeps the outer `(` line
	x = (
		a
	): ( // g15
		B | C // x15
	) => 1;

	// a directive in a member's stripped pair leads the member, which it freezes
	x = (
		a
	): (
		| B
		// prettier-ignore
		| C // x16
	) => 1;

	// a directive ahead of a member's own pair freezes the pair as written
	x = (
		a
	):
		| B
		// prettier-ignore
		| (C[] // x17
	) => 1;
</script>
