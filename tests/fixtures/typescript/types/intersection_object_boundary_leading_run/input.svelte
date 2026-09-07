<script lang="ts">
	// An object-adjacent boundary breaks and indents when the member's leading run ENDS A
	// LINE — prettier's `hasLeadingOwnLineComment` disjunct on the "no object is involved"
	// arm. Without it the boundary hugs, leaving the run no line to flush at and carrying
	// it out past the `;`
	type A = a & // c1
		// c2
		{ x: X };

	// the run reaches the boundary the same way when the author wrote it past the `&`
	type B = a &
		// c3
		{ x: X };

	// a third member past the object keeps its hug — only this boundary opens
	type C = a & // c4
		// c5
		{ x: X } & b;

	// and a shell on a LATER member, whose boundary is object-adjacent all the same
	type D = b &
		a & // c6
		// c7
		{ x: X };
</script>
