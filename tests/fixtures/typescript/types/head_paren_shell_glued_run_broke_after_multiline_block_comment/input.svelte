<script lang="ts">
	// A run the author GLUED — a multi-line block ahead of a single-line one — and then
	// broke after, inside a redundant paren shell: the shell strips and the run lays out
	// as the shell-free authoring does, the value dropping below the run's closing `*/`.

	// alias `=`
	type A =
		/* c1
	d1 */ /* c2 */
		B;

	// annotation `:`
	let c: /* c3
	d3 */ /* c4 */
	D;

	// return type
	function e(): /* c5
	d5 */ /* c6 */
	F {}

	// parameter annotation
	function g(
		h: /* c7
	d7 */ /* c8 */
		I
	) {}

	// first tuple element
	type J = [
		/* c9
	d9 */ /* c10 */
		K,
		L
	];

	// first union member
	type M =
		| /* c11
	d11 */ /* c12 */
		  N
		| O;

	// sole type argument
	type P = Foo<
		/* c13
	d13 */ /* c14 */
		Q
	>;

	// conditional `?` branch
	type R = S extends T
		? /* c15
	d15 */ /* c16 */
			U
		: V;

	// conditional `:` branch
	type W = X extends Y
		? Z
		: /* c17
	d17 */ /* c18 */
			AA;

	// author blank below the run: first tuple element
	type FF = [
		/* c23
	d23 */ /* c24 */

		GG,
		HH
	];

	// author blank below the run: sole type argument
	type II = Foo<
		/* c25
	d25 */ /* c26 */

		JJ
	>;

	// author blank below the run: first union member
	type KK =
		| /* c27
	d27 */ /* c28 */

		  LL
		| MM;

	// author blank below the run: intersection later member
	type NN = OO &
		/* c29
	d29 */ /* c30 */

		PP;

	// author blank below the run: tuple rest element
	type QQ = [
		.../* c31
	d31 */ /* c32 */

		RR[]
	];

	// control: the whole run glued to the value keeps the value on the closing line
	type BB = /* c19
	d19 */ /* c20 */ CC;

	// control: a single-line run the author broke after collapses onto the value
	type DD = /* c21 */ /* c22 */ EE;
</script>
