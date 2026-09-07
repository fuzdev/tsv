<script lang="ts">
	// A block run at the member's leading printed EDGE, one link down under an array suffix
	type A =
		| /* c1 */ B[] // c2
		  // c3
		| c;

	// the line-comment spelling of the same edge
	type D =
		| // c4
		  B[] // c5
		  // c6
		| c;

	// the same edge under an indexed access
	type G =
		| /* c7 */ B['k'] // c8
		  // c9
		| c;

	// a LATER member, whose inline (block) edge run still leads it
	type J =
		| z
		| /* c10 */ B[] // c11
		  // c12
		| c;

	// a later member whose OWN-LINE edge run is relocated above the `|`, where it leads
	// nothing
	type M =
		| z // c13
		| B[] // c14
		// c15
		| c;
</script>
