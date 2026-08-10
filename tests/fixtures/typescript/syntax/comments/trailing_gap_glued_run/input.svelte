<script lang="ts">
	// A pair the author GLUED onto one line keeps that line in a trailing gap, the same
	// rule an end-of-body run reads (syntax/comments/body_trailing_glued_run). The
	// multiline block ahead of the pair is what puts it on a line the item never touched.

	// Tuple element.
	type Tup = [
		A /* c1
c2 */ /* c3 */
		// c4
	];

	// Type parameter.
	function fn<
		T /* c1
c2 */ /* c3 */
		// c4
	>() {}

	// Value parameter.
	function fn2(
		a /* c1
c2 */ /* c3 */
		// c4
	) {}

	// Function-type parameter.
	type F = (
		a: A /* c1
c2 */ /* c3 */
		// c4
	) => void;

	// Union member, before the `|`.
	type U =
		| A /* c1
c2 */ /* c3 */
		// c4
		| B;

	// Control - a pair glued on the item's own line trails it under either reading.
	type Tup2 = [
		A /* c1 */ /* c2 */
		// c3
	];

	// Control - two lines the author gave two comments stay two lines.
	type Tup3 = [
		A /* c1 */
		/* c2 */
		// c3
	];
</script>
