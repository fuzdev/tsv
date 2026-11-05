<script lang="ts">
	// a comment written last in a pair down the right edge of an arrow's parenthesized
	// return type stays inside the outer pair, as it does when written there
	x = (
		a
	):
		B | C /* x1
		 */ => 1;

	x = (
		a
	):
		B | C => 1; // x2

	x = (
		a
	): B & C /* x3
	 */ => 1;

	x = (
		a
	): keyof C /* x4
	 */ => 1;

	x = (
		a
	): A extends B ? C : D /* x5
	 */ => 1;

	x = (
		a
	):
		B | C /* x6
		 */ => 1;

	// an inner pair that could stay is no authoring signal either
	x = (
		a
	):
		B | C[] => 1; // x7

	// a `//` after the `:` hangs the kept pair below it
	x = (
		a
	):
		// p8
		B | C /* x8
		 */ => 1;

	// a pair the printer adds keeps the comment after it
	x = (
		a
	):
		B | (C & D) => 1; // x9

	// a generic reference keeps the outer pair, which a `(` cannot misread
	x = (
		a
	): G<T> => 1; // x10

	// a comment leading the inner pair stays ahead of the member
	x = (
		a
	):
		| B
		// y11
		| C /* x11
		 */ => 1;

	// an operator member and a constructor type's return type keep the outer pair too
	x = (
		a
	):
		B | keyof C /* x12
		 */ => 1;

	x = (
		a
	):
		B | readonly C[] => 1; // x13

	x = (
		a
	): new () => C => 1; // x14

	// a `//` glued to the inner `(` keeps the outer `(` line
	x = (
		a
	):
		// g15
		B | C => 1; // x15

	// a directive in a member's stripped pair leads the member, which it freezes
	x = (
		a
	):
		| B
		// prettier-ignore
		| C => 1; // x16

	// a directive ahead of a member's own pair freezes the pair as written
	x = (
		a
	):
		| B
		// prettier-ignore
		| C[] => 1; // x17
</script>
