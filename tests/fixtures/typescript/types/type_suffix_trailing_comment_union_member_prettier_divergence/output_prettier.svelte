<script lang="ts">
	// an indexed access as a union member keeps a trailing line comment inside
	// its brackets, like every bracketed type region
	type A1 =
		| T[K] // c1
		| B;

	// the LAST member's redundant paren shell is retained: no separator follows
	// it, so a stripped shell's comment would escape past the `;`
	type A2 =
		| B
		| A; // c2

	type A3 = B &
		A; // c3

	// a sole parenthesized member is retained the same way (the one-member
	// union's `|` drops — unformatted_ours_flat carries that authoring)
	type A4 = A; // c4

	// the retain rule is the SHELL's, and a LEADING run in the same shell does not
	// strip it either — the trailing `//` would escape past the `;` just the same
	type A5 =
		| B
		// c5
		| A; // c6
		// c7

	// the leading run may sit one link down, at the member's leading printed EDGE:
	// the shells strip as a unit, so the retained pair holds both runs
	type A6 =
		| B
		// c8
		| A[]; // c9
		// c10

	// the intersection's last member, which already answered this
	type A7 = B &
		// c11
		A; // c12
		// c13

	// a sole member of either composite is retained whatever its inner — a union...
	type A8 =
		A | B; // c14
		// c15

	// ...and a required pair
	type A9 = (a: 1) => void; // c16
	// c17

	// the leading run keeps the `(`'s line where the author glued it there — the
	// opening-delimiter rule, so the two authorings are two fixed points
	type A10 =
		| B // c18
		| A; // c19
		// c20

	type A11 = B & // c21
		A; // c22
		// c23
</script>
