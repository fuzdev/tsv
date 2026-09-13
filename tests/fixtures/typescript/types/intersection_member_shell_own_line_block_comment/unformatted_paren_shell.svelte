<script lang="ts">
	// an own-line BLOCK inside an INTERSECTION member's stripped paren shell keeps its own
	// line, exactly as the `//` spelling and the shell-free authoring do — the seam's union
	// twin, one member over

	// a later member
	type A = B & (
	/* c1 */
	C);

	// the member is an object type
	type D = E & (
	/* c2 */
	{ a: 1 });

	// two own-line blocks, each keeping its own line
	type F = G & (
	/* c3 */
	/* c4 */
	H);

	// the second block glued to the member stays glued to it
	type I = J & (
	/* c5 */
	/* c6 */ K);

	// a middle member, so the run is neither the first nor the last
	type R = S & (
	/* c7 */
	T) & U;

	// control: the `//` spelling, which lands on the same shape
	type L = M & (
	// c8
	N);

	// control: a block the author glued to the `(` stays inline
	type O = P & ( /* c9 */ Q);

	// an OBJECT on the other side of the boundary, which hugs by default — the run still
	// has to open and INDENT it, or the member lands at column zero
	type Xa = { a: 1 } & (
	/* c10 */
	Xb);

	// the same past a TRANSPARENT composite's dropped operator, which is the window the
	// boundary rule could not see
	type Xc = { a: 1 } & (
	|
	/* c11 */
	Xd);

	// and its `//` spelling
	type Xe = { a: 1 } & (
	|
	// c12
	Xf);
</script>
