<script lang="ts">
	// the operator of a TRANSPARENT composite inside a member's stripped shell does not
	// print, so the two newlines bracketing it are not an author blank line — the line it
	// sits on is not blank, it holds a token

	// the dropped `|`
	type A = B | (
	/* c1 */
	|
	/* d1 */
	C);

	// the dropped `&`
	type D = E | (
	/* c2 */
	&
	/* d2 */
	F);

	// control: an author blank between two comments in the same run survives
	type G = H | (
	/* c3 */

	/* d3 */
	I);

	// control: the two together — the dropped operator's gap is not a blank, the author's is
	type J = K | (
	/* c4 */
	|
	/* d4 */

	/* e4 */
	L);

	// the `//`-led run, whose shell sits at an INTERSECTION member — the same dropped
	// operator, a different emitter
	type M = N & (
	// x1
	|
	/* d5 */
	O);

	// and at a union member
	type P = Q | (
	// x2
	&
	/* d6 */
	R);
</script>
