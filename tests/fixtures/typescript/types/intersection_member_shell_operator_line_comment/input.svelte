<script lang="ts">
	// a `//` the author left on the `&`'s own line, inside an intersection member's
	// stripped paren shell, keeps that line — the `(` between them does not print, so the
	// line the comment was written on IS the operator's

	// a later member
	type A = B & // c1
		C;

	// the rest of the run keeps its own lines below
	type D = E & // c2
		// c3
		F;

	// the rest is a block the author gave its own line
	type G = H & // c4
		/* c5 */
		I;

	// a middle member, so the run is neither the first nor the last
	type J = K & // c6
		L &
		M;

	// control: a block is never pulled onto the operator's line — it renders inline
	type P = Q & /* c8 */ R;
</script>
