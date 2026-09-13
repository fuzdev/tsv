<script lang="ts">
	// A multiline block comment at the end of a redundant paren shell keeps the shell
	// wherever the token after it may not follow a line break, and the kept shell
	// expands: a conditional type's `extends`
	type A = (
		B /* m1
	m2 */
	) extends C
		? D
		: E;

	// a nested conditional's check type
	type F = G extends H
		? (
			I /* m3
	m4 */
		) extends J
			? K
			: L
		: M;

	// an optional tuple element's `?`
	type N = [
		(
			O /* m5
	m6 */
		)?
	];
	type P = [
		(
			{ x: X } /* m7
	m8 */
		)?
	];

	// an array type's `[]` and an indexed access's `[`
	type Q = (
		R /* m9
	m10 */
	)[];
	type S = (
		T /* m11
	m12 */
	)['k'];
	type U = {
		[V in string]: (
			W /* m13
	m14 */
		)[];
	};

	// Where nothing restricted follows, the shell strips and the comment trails inline.
	type Y = Z /* m15
	m16 */;
	type A1 =
		| B1 /* m17
	m18 */
		| C1;

	// A single-line block strips the shell everywhere, as before.
	type D1 = E1 /* c1 */ extends F1 ? G1 : H1;
	type I1 = J1 /* c2 */[];
</script>
