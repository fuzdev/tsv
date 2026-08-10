<script lang="ts">
	// A comment the author GLUED to the previous trailing comment's `*/` trails the member
	// it was written behind, the same rule the union's member gap reads. A block stays on
	// its side of the `&`; a line comment trails the operator, which is the only lossless
	// place for it.

	// Block comment glued past a multiline block, before the `&`.
	type I = A /* c1
c2 */ /* c3 */ &
		// c4
		B;

	// Line comment glued past a multiline block trails the `&`.
	type J = A /* c1
c2 */ & // c3
		B;

	// Control - a comment the author gave a line of its own keeps it.
	type K = A /* c1
c2 */ &
		/* c3 */
		B;

	// A blank the author left before the comment does not cross the `&` with it; one
	// between two comments is theirs and stays.
	type M = A &
		// c1

		// c2
		B;

	// Control - the union's member gap answers both the same way.
	type L =
		| A /* c1
c2 */ /* c3 */
		// c4
		| B;
</script>
