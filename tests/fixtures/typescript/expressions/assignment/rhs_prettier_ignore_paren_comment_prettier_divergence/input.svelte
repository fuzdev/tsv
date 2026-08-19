<script lang="ts">
	// A frozen assignment RHS answers the slice→`)` gap exactly as a frozen declarator
	// initializer does — one rule, two hosts. A `//` retains the shell on its own account.
	aaa =
		// prettier-ignore
		(
			bbb  =  ccc // c
		);

	// A frozen sequence supplies its own required parens, and the gap is inside them.
	ddd =
		// prettier-ignore
		(eee,  fff /* c */);

	// A shell the frozen value does not need strips, and the block defers past the `;`.
	ggg =
		// prettier-ignore
		hhh  =  iii; /* c */
</script>
