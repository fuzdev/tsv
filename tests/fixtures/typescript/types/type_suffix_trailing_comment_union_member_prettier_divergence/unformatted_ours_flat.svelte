<script lang="ts">
	// an indexed access as a union member keeps a trailing line comment inside
	// its brackets, like every bracketed type region
	type A1 = T[K // c1
	] | B;

	// the LAST member's redundant paren shell is retained: no separator follows
	// it, so a stripped shell's comment would escape past the `;`
	type A2 = B | (A // c2
	);

	type A3 = B & (A // c3
	);

	// a sole parenthesized member is retained the same way (the one-member
	// union's `|` drops — unformatted_ours_flat carries that authoring)
	type A4 = | (A // c4
	);
</script>
