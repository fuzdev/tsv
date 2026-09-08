<script lang="ts">
	// A leading-edge paren shell whose enclosing gap is a RETAINED shell's own interior:
	// the outer pair keeps its parens for its trailing run, and the inner shell strips,
	// so the `//` lands in the outer shell's leading region and that region's emitter
	// prints it — outside every group the inner type builds
	type A = z &
		// c1
		(a extends b ? c : d); // c2
		// c3

	// a nested intersection inner, whose members level from the shell's own interior
	type B = z &
		// c4
		(a & b); // c5
		// c6

	// an array suffix, the descent's first link
	type C = z &
		// c7
		a[]; // c8
		// c9

	// an indexed access, its second
	type D = z &
		// c10
		a['k']; // c11
		// c12

	// a run the author GLUED to the `(` keeps that line, at the surviving pair — the
	// opening-delimiter rule, so the two authorings are two fixed points
	type E = z & // c13
		(a extends b ? c : d); // c14
		// c15
</script>
